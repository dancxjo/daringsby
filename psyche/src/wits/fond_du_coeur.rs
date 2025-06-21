use crate::{
    Impression, Summarizer,
    ling::{Doer, Instruction},
    wit::Moment,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Summarizes recent `Moment`s into a rolling life story.
#[derive(Clone)]
pub struct FondDuCoeur {
    doer: Arc<dyn Doer>,
    story: Arc<Mutex<String>>,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl FondDuCoeur {
    /// Create a new `FondDuCoeur` using the provided [`Doer`].
    pub fn new(doer: Box<dyn Doer>) -> Self {
        Self {
            doer: doer.into(),
            story: Arc::new(Mutex::new(String::new())),
            tx: None,
        }
    }

    /// Create a `FondDuCoeur` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(doer: Box<dyn Doer>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            doer: doer.into(),
            story: Arc::new(Mutex::new(String::new())),
            tx: Some(tx),
        }
    }

    /// Return the most recently generated story.
    pub fn story(&self) -> String {
        self.story.lock().unwrap().clone()
    }
}

#[async_trait]
impl Summarizer<Moment, String> for FondDuCoeur {
    async fn digest(&self, inputs: &[Impression<Moment>]) -> anyhow::Result<Impression<String>> {
        let mut combined = self.story();
        for imp in inputs {
            if !combined.is_empty() {
                combined.push(' ');
            }
            combined.push_str(&imp.raw_data.summary);
        }
        let instruction = Instruction {
            command: format!("Summarize Pete's life story in one paragraph:\n{combined}"),
            images: Vec::new(),
        };
        let resp = self.doer.follow(instruction.clone()).await?;
        let summary = resp.trim().to_string();
        *self.story.lock().unwrap() = summary.clone();
        if let Some(tx) = &self.tx {
            let _ = tx.send(crate::WitReport {
                name: "Story".into(),
                prompt: instruction.command.clone(),
                output: summary.clone(),
            });
        }
        Ok(Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: summary.clone(),
            details: None,
            raw_data: summary,
        })
    }
}
