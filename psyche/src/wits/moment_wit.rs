use crate::topics::{Topic, TopicBus};
use crate::traits::Doer;
use crate::{Impression, Stimulus, WitReport};
use async_trait::async_trait;
use futures::StreamExt;
use lingproc::Instruction;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::debug;

/// Wit summarizing recent instant impressions into a single moment.
pub struct MomentWit {
    buffer: Arc<Mutex<Vec<String>>>,
    bus: TopicBus,
    doer: Arc<dyn Doer>,
    tx: Option<broadcast::Sender<WitReport>>,
}

impl MomentWit {
    /// Debug label for filtering reports.
    pub const LABEL: &'static str = "MomentWit";

    /// Create a new `MomentWit` subscribed to `bus` using `doer`.
    pub fn new(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(bus, doer, None)
    }

    /// Create a `MomentWit` that emits [`WitReport`]s via `tx`.
    pub fn with_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<WitReport>>,
    ) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = buffer.clone();
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let mut stream = bus_clone.subscribe(Topic::Instant);
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
            tx,
        }
    }
}

#[async_trait]
impl crate::wit::Wit<(), String> for MomentWit {
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
        debug!(count = items.len(), "moment wit summarizing instants");
        let prompt = format!("Summarize these recent events:\n- {}", items.join("\n- "));
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
                debug!(?e, "moment wit doer failed");
                String::new()
            }
        };
        if resp.is_empty() {
            return Vec::new();
        }
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
        self.bus.publish(Topic::Moment, imp.clone());
        vec![imp]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
