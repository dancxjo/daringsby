use crate::instruction::{Instruction, parse_instructions};
use crate::ling::{Doer, Instruction as LlmInstruction};
use crate::prompt::{PromptBuilder, WillPrompt};
use crate::topics::{Topic, TopicBus};
use crate::{Impression, Stimulus, WitReport};
use async_trait::async_trait;
use futures::StreamExt;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::debug;

/// Wit that decides Pete's next action and publishes [`Instruction`]s.
pub struct WillWit {
    bus: TopicBus,
    doer: Arc<dyn Doer>,
    prompt: WillPrompt,
    buffer: Arc<Mutex<Vec<String>>>,
    history: Arc<Mutex<Vec<Instruction>>>,
    tx: Option<broadcast::Sender<WitReport>>,
}

impl WillWit {
    /// Debug label used for debugging filters.
    pub const LABEL: &'static str = "WillWit";

    /// Create a new `WillWit` subscribed to `bus` using `doer`.
    pub fn new(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(bus, doer, None)
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: WillPrompt) {
        self.prompt = prompt;
    }

    /// Create a new `WillWit` emitting [`WitReport`]s via `tx`.
    pub fn with_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<WitReport>>,
    ) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let history = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = buffer.clone();
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let inst = bus_clone.subscribe(Topic::Instant);
            let mom = bus_clone.subscribe(Topic::Moment);
            tokio::pin!(inst);
            tokio::pin!(mom);
            loop {
                tokio::select! {
                    Some(p) = inst.next() => {
                        if let Ok(i) = Arc::downcast::<Impression<String>>(p) {
                            buf_clone.lock().unwrap().push(i.summary.clone());
                        }
                    }
                    Some(p) = mom.next() => {
                        if let Ok(i) = Arc::downcast::<Impression<String>>(p) {
                            buf_clone.lock().unwrap().push(i.summary.clone());
                        }
                    }
                }
            }
        });
        Self {
            bus,
            doer,
            prompt: WillPrompt,
            buffer,
            history,
            tx,
        }
    }
}

#[async_trait]
impl crate::wit::Wit<(), Instruction> for WillWit {
    async fn observe(&self, _: ()) {}

    async fn tick(&self) -> Vec<Impression<Instruction>> {
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            let out = buf.join(" \n");
            buf.clear();
            out
        };
        let prompt_text = self.prompt.build(&items);
        let resp = match self
            .doer
            .follow(LlmInstruction {
                command: prompt_text.clone(),
                images: Vec::new(),
            })
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!(?e, "will wit doer failed");
                return Vec::new();
            }
        };
        let instructions = parse_instructions(&resp);
        let unique = {
            let mut hist = self.history.lock().unwrap();
            let mut unique = Vec::new();
            for ins in instructions {
                if hist.last().map_or(false, |h| h == &ins) {
                    continue;
                }
                self.bus.publish(Topic::Instruction, ins.clone());
                hist.push(ins.clone());
                if hist.len() > 10 {
                    hist.remove(0);
                }
                unique.push(ins);
            }
            unique
        };
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(WitReport {
                    name: Self::LABEL.into(),
                    prompt: prompt_text,
                    output: resp.clone(),
                });
            }
        }
        unique
            .into_iter()
            .map(|ins| {
                Impression::new(
                    vec![Stimulus::new(ins.clone())],
                    format!("{:?}", ins),
                    None::<String>,
                )
            })
            .collect()
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
