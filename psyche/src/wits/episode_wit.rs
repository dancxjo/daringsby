use crate::Instruction;
use crate::topics::{Topic, TopicBus};
use crate::traits::Doer;
use crate::{Impression, Stimulus, WitReport};
use async_trait::async_trait;
use futures::StreamExt;
use lingproc::Instruction as LlmInstruction;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::broadcast;
use tracing::debug;

/// Wit that groups situations into higher-level narrative episodes.
pub struct EpisodeWit {
    buffer: Arc<Mutex<Vec<String>>>,
    bus: TopicBus,
    doer: Arc<dyn Doer>,
    break_flag: Arc<AtomicBool>,
    tx: Option<broadcast::Sender<WitReport>>,
}

impl EpisodeWit {
    /// Debug label for filtering reports.
    pub const LABEL: &'static str = "EpisodeWit";

    /// Create a new `EpisodeWit` subscribed to `bus` using `doer`.
    pub fn new(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(bus, doer, None)
    }

    /// Create an `EpisodeWit` that emits [`WitReport`]s via `tx`.
    pub fn with_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<WitReport>>,
    ) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let break_flag = Arc::new(AtomicBool::new(false));
        let buf_clone = buffer.clone();
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let mut stream = bus_clone.subscribe(Topic::Situation);
            tokio::pin!(stream);
            while let Some(payload) = stream.next().await {
                if let Ok(i) = Arc::downcast::<Impression<String>>(payload) {
                    buf_clone.lock().unwrap().push(i.summary.clone());
                }
            }
        });

        let break_clone = break_flag.clone();
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let mut stream = bus_clone.subscribe(Topic::Instruction);
            tokio::pin!(stream);
            while let Some(payload) = stream.next().await {
                if let Ok(i) = Arc::downcast::<Instruction>(payload) {
                    if matches!(*i, Instruction::BreakEpisode) {
                        break_clone.store(true, Ordering::SeqCst);
                    }
                }
            }
        });

        Self {
            buffer,
            bus,
            doer,
            break_flag,
            tx,
        }
    }
}

#[async_trait]
impl crate::wit::Wit<(), String> for EpisodeWit {
    async fn observe(&self, _: ()) {}

    async fn tick(&self) -> Vec<Impression<String>> {
        const MIN_ITEMS: usize = 3;
        let should_break = self.break_flag.swap(false, Ordering::SeqCst);
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            if buf.len() < MIN_ITEMS && !should_break {
                return Vec::new();
            }
            buf.drain(..).collect::<Vec<_>>()
        };
        debug!(count = items.len(), "episode wit summarizing situations");
        let prompt = format!(
            "The following situations form a coherent story. Write a one-sentence summary suitable for a chapter heading.\n- {}",
            items.join("\n- ")
        );
        let resp = match self
            .doer
            .follow(LlmInstruction {
                command: prompt.clone(),
                images: Vec::new(),
            })
            .await
        {
            Ok(s) => s.trim().to_string(),
            Err(e) => {
                debug!(?e, "episode wit doer failed");
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
        self.bus.publish(Topic::Episode, imp.clone());
        vec![imp]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
