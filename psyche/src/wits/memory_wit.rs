use crate::wit::{Moment, Wit};
use crate::{Impression, wits::Memory};
use async_trait::async_trait;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::broadcast;
use tracing::{debug, error};

/// Wit that aggregates text impressions into brief [`Moment`] summaries.
///
/// Collected impressions are periodically concatenated and stored in the
/// provided [`Memory`] implementation. A [`WitReport`] is emitted whenever a
/// summary is created.
pub struct MemoryWit {
    memory: Arc<dyn Memory>,
    buffer: Mutex<Vec<Impression<String>>>,
    collected: Mutex<Vec<Impression<String>>>,
    ticks: AtomicUsize,
    threshold: usize,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl MemoryWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "Memory";
    /// Create a new `MemoryWit` using `memory` as the storage backend.
    pub fn new(memory: Arc<dyn Memory>) -> Self {
        Self {
            memory,
            buffer: Mutex::new(Vec::new()),
            collected: Mutex::new(Vec::new()),
            ticks: AtomicUsize::new(0),
            threshold: 5,
            tx: None,
        }
    }

    /// Create a `MemoryWit` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(memory: Arc<dyn Memory>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            tx: Some(tx),
            ..Self::new(memory)
        }
    }
}

#[async_trait]
impl Wit<Impression<String>, Moment> for MemoryWit {
    async fn observe(&self, input: Impression<String>) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<Moment>> {
        let new_items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                Vec::new()
            } else {
                buf.drain(..).collect::<Vec<_>>()
            }
        };
        {
            let mut collected = self.collected.lock().unwrap();
            collected.extend(new_items);
        }
        let count = self.ticks.fetch_add(1, Ordering::SeqCst) + 1;
        let should_summarize = {
            let c = self.collected.lock().unwrap();
            !c.is_empty() && (c.len() >= self.threshold || count >= self.threshold)
        };
        if !should_summarize {
            return Vec::new();
        }
        self.ticks.store(0, Ordering::SeqCst);
        let items = {
            let mut coll = self.collected.lock().unwrap();
            let data = coll.clone();
            coll.clear();
            data
        };
        // create summary
        let summary = items
            .iter()
            .map(|i| i.headline.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let moment = Moment {
            summary: summary.clone(),
        };
        let impression = Impression::new(summary.clone(), None::<String>, moment.clone());
        if let Err(e) = self.memory.store_serializable(&impression).await {
            error!(?e, "failed to store memory summary");
        }
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: "naive concat".into(),
                    output: summary.clone(),
                });
            }
        }
        debug!("memory summarized {} impressions", items.len());
        vec![impression]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
