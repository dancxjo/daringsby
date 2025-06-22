use crate::{
    Impression, Motor, Stimulus,
    ling::{Doer, Instruction},
    wit::Wit,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Wit analyzing feelings and updating Pete's emotional state.
pub struct HeartWit {
    doer: Arc<dyn Doer>,
    motor: Arc<dyn Motor>,
    buffer: Mutex<Vec<Impression<String>>>,
    instants: Mutex<Vec<Arc<crate::Instant>>>,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl HeartWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "Heart";
    /// Create a new `HeartWit` using the given LLM `doer` and host `motor`.
    pub fn new(doer: Box<dyn Doer>, motor: Arc<dyn Motor>) -> Self {
        Self {
            doer: doer.into(),
            motor,
            buffer: Mutex::new(Vec::new()),
            instants: Mutex::new(Vec::new()),
            tx: None,
        }
    }

    /// Create a `HeartWit` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(
        doer: Box<dyn Doer>,
        motor: Arc<dyn Motor>,
        tx: broadcast::Sender<crate::WitReport>,
    ) -> Self {
        Self {
            tx: Some(tx),
            ..Self::new(doer, motor)
        }
    }
}

#[async_trait]
impl Wit<Impression<String>, String> for HeartWit {
    async fn observe(&self, input: Impression<String>) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<String>> {
        let inputs = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            let data = buf.clone();
            buf.clear();
            data
        };
        let summary = inputs
            .iter()
            .flat_map(|i| i.stimuli.iter().map(|s| s.what.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        let instruction = Instruction {
            command: format!("What emoji reflects Pete's mood? {summary}"),
            images: Vec::new(),
        };
        let prompt = instruction.command.clone();
        let resp = match self.doer.follow(instruction).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let mood = resp.trim().to_string();
        self.motor.set_emotion(&mood).await;
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt,
                    output: resp.clone(),
                });
            }
        }
        vec![Impression::new(
            vec![Stimulus::new(mood.clone())],
            summary,
            Some(mood),
        )]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

#[async_trait]
impl crate::traits::observer::SensationObserver for HeartWit {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(instant) = payload.downcast_ref::<Arc<crate::Instant>>() {
            self.instants.lock().unwrap().push(instant.clone());
        }
    }
}
