use crate::topics::{Topic, TopicBus};
use crate::{Impression, Stimulus, WitReport};
use async_trait::async_trait;
use futures::StreamExt;
use lingproc::{Doer, Instruction};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::debug;

/// Wit that summarizes recent moments into an ongoing situation.
pub struct SituationWit {
    buffer: Arc<Mutex<Vec<String>>>,
    bus: TopicBus,
    doer: Arc<dyn Doer>,
    last: Arc<Mutex<Option<String>>>,
    tx: Option<broadcast::Sender<WitReport>>,
}

impl SituationWit {
    /// Debug label for filtering reports.
    pub const LABEL: &'static str = "SituationWit";

    /// Create a new `SituationWit` subscribed to `bus` using `doer`.
    pub fn new(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(bus, doer, None)
    }

    /// Create a `SituationWit` that emits [`WitReport`]s via `tx`.
    pub fn with_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<WitReport>>,
    ) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let last = Arc::new(Mutex::new(None));
        let buf_clone = buffer.clone();
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let mut stream = bus_clone.subscribe(Topic::Moment);
            tokio::pin!(stream);
            while let Some(payload) = stream.next().await {
                if let Ok(i) = Arc::downcast::<Impression<String>>(payload) {
                    buf_clone.lock().unwrap().push(i.summary.clone());
                }
            }
        });
        Self {
            buffer,
            bus,
            doer,
            last,
            tx,
        }
    }
}

#[async_trait]
impl crate::wit::Wit<(), String> for SituationWit {
    async fn observe(&self, _: ()) {}

    async fn tick(&self) -> Vec<Impression<String>> {
        const MIN_ITEMS: usize = 3;
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.len() < MIN_ITEMS {
                return Vec::new();
            }
            buf.drain(..).collect::<Vec<_>>()
        };
        debug!(count = items.len(), "situation wit summarizing moments");
        let previous = self.last.lock().unwrap().clone();
        let mut prompt = String::new();
        if let Some(prev) = previous {
            if !prev.trim().is_empty() {
                prompt.push_str(&format!("Previous situation: {}\n", prev));
            }
        }
        prompt.push_str("Given the following recent moments, summarize the ongoing situation in one sentence:\n- ");
        prompt.push_str(&items.join("\n- "));
        let resp = match self
            .doer
            .follow(Instruction {
                command: prompt.clone(),
                images: Vec::new(),
            })
            .await
        {
            Ok(s) => s.trim().to_string(),
            Err(e) => {
                debug!(?e, "situation wit doer failed");
                String::new()
            }
        };
        if resp.is_empty() {
            return Vec::new();
        }
        *self.last.lock().unwrap() = Some(resp.clone());
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(WitReport {
                    name: Self::LABEL.into(),
                    prompt: prompt.clone(),
                    output: resp.clone(),
                });
            }
        }
        let imp = Impression::new(vec![Stimulus::new(resp.clone())], resp, None::<String>);
        self.bus.publish(Topic::Situation, imp.clone());
        vec![imp]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
