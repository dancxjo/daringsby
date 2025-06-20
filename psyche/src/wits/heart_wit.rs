use crate::{
    Impression, Motor,
    ling::{Doer, Instruction},
    wit::Wit,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// Wit analyzing feelings and updating Pete's emotional state.
pub struct HeartWit {
    doer: Arc<dyn Doer>,
    motor: Arc<dyn Motor>,
    buffer: Mutex<Vec<Impression<String>>>,
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
            .map(|i| i.raw_data.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let instruction = Instruction {
            command: format!("What emoji reflects Pete's mood? {summary}"),
            images: Vec::new(),
        };
        let resp = match self.doer.follow(instruction).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let mood = resp.trim().to_string();
        self.motor.set_emotion(&mood).await;
        vec![Impression::new(mood.clone(), Some(summary), mood)]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
