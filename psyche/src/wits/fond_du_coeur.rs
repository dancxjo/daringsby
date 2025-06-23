use crate::traits::Doer;
use crate::{Impression, Stimulus};
use lingproc::Instruction;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Summarizes recent `Moment`s into a rolling life story.
#[derive(Clone)]
pub struct FondDuCoeur {
    doer: Arc<dyn Doer>,
    story: Arc<Mutex<String>>,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl FondDuCoeur {
    /// Debug label for this summarizer.
    pub const LABEL: &'static str = "Story";
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

impl FondDuCoeur {
    /// Summarize recent moments into a single life story paragraph.
    pub async fn digest(
        &self,
        inputs: &[Impression<String>],
    ) -> anyhow::Result<Impression<String>> {
        let mut combined = self.story();
        for imp in inputs {
            if let Some(stim) = imp.stimuli.first() {
                if !combined.is_empty() {
                    combined.push(' ');
                }
                combined.push_str(&stim.what);
            }
        }
        let instruction = Instruction {
            command: format!("Summarize Pete's life story in one paragraph:\n{combined}"),
            images: Vec::new(),
        };
        let resp = self.doer.follow(instruction.clone()).await?;
        let summary = resp.trim().to_string();
        *self.story.lock().unwrap() = summary.clone();
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: instruction.command.clone(),
                    output: summary.clone(),
                });
            }
        }
        Ok(Impression::new(
            vec![Stimulus::new(summary.clone())],
            summary,
            None::<String>,
        ))
    }
}
