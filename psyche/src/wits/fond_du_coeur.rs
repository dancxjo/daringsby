use crate::traits::Doer;
use crate::{Impression, Stimulus};
use lingproc::LlmInstruction;
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
        let previous_story = self.story();
        let recent = inputs
            .iter()
            .filter_map(|imp| imp.stimuli.first().map(Stimulus::prompt_list_item))
            .collect::<Vec<_>>();
        let mut context = String::new();
        if !previous_story.trim().is_empty() {
            context.push_str("Previous story:\n");
            context.push_str(&previous_story);
            context.push('\n');
        }
        if !recent.is_empty() {
            context.push_str("Recent moments:\n- ");
            context.push_str(&recent.join("\n- "));
        }
        let instruction = LlmInstruction {
            command: crate::with_default_system_prompt(format!(
                "Summarize Pete's life story in one paragraph:\n{context}"
            )),
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
            vec![Stimulus::from_impressions(summary.clone(), inputs)],
            summary,
            None::<String>,
        ))
    }
}
