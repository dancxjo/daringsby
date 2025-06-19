use crate::prompt::PromptBuilder;
use crate::{
    Impression, Summarizer,
    ling::{Doer, Instruction},
    wit::Episode,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Summarizes recent [`Episode`]s into a short awareness statement.
///
/// The resulting sentence feeds into [`Will`] to inform the next action.
///
/// # Example
/// ```no_run
/// # use psyche::{wits::Combobulator, ling::{Doer, Instruction}, Impression, Summarizer, wit::Episode};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Doer for Dummy {
/// #   async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
/// #       Ok("All clear.".to_string())
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let combo = Combobulator::new(Box::new(Dummy));
/// let imp = combo
///     .digest(&[Impression::new("", None::<String>, Episode { summary: "Pete looked around".into() })])
///     .await?;
/// assert_eq!(imp.raw_data, "All clear.");
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Combobulator {
    doer: Arc<dyn Doer>,
    prompt: crate::prompt::CombobulatorPrompt,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl Combobulator {
    /// Create a new `Combobulator` using the provided [`Doer`].
    pub fn new(doer: Box<dyn Doer>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::CombobulatorPrompt,
            tx: None,
        }
    }

    /// Create a `Combobulator` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(doer: Box<dyn Doer>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::CombobulatorPrompt,
            tx: Some(tx),
        }
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: crate::prompt::CombobulatorPrompt) {
        self.prompt = prompt;
    }
}

#[async_trait]
impl Summarizer<Episode, String> for Combobulator {
    async fn digest(&self, inputs: &[Impression<Episode>]) -> anyhow::Result<Impression<String>> {
        let mut combined = String::new();
        for imp in inputs {
            if !combined.is_empty() {
                combined.push(' ');
            }
            combined.push_str(&imp.raw_data.summary);
        }
        let instruction = Instruction {
            command: self.prompt.build(&combined),
            images: Vec::new(),
        };
        let resp = self.doer.follow(instruction.clone()).await?;
        let summary = resp.trim().to_string();
        if let Some(tx) = &self.tx {
            let _ = tx.send(crate::WitReport {
                name: "Combobulator".into(),
                prompt: instruction.command.clone(),
                output: summary.clone(),
            });
        }
        Ok(Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: summary.clone(),
            details: Some(combined),
            raw_data: summary,
        })
    }
}
