use crate::prompt::PromptBuilder;
use crate::{
    Impression, Summarizer,
    ling::{Doer, Instruction},
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Decide Pete's next action or speech using a language model.
///
/// `Will` sends the given situation summary to a [`Doer`] with a
/// brief prompt asking for a single sentence describing what Pete
/// should do or say next. The decision is returned as an
/// [`Impression`].
///
/// # Example
/// ```no_run
/// # use psyche::{Will, ling::{Doer, Instruction}, Impression, Summarizer};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Doer for Dummy {
/// #   async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
/// #       Ok("Speak.".to_string())
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() {
/// let will = Will::new(Box::new(Dummy));
/// let imp = will
///     .digest(&[Impression::new("", None::<String>, "greet the user".to_string())])
///     .await
///     .unwrap();
/// assert_eq!(imp.raw_data, "Speak.");
/// # }
/// ```
#[derive(Clone)]
pub struct Will {
    doer: Arc<dyn Doer>,
    prompt: crate::prompt::WillPrompt,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl Will {
    /// Create a new `Will` using the provided [`Doer`].
    pub fn new(doer: Box<dyn Doer>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::WillPrompt::default(),
            tx: None,
        }
    }

    /// Create a `Will` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(doer: Box<dyn Doer>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::WillPrompt::default(),
            tx: Some(tx),
        }
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: crate::prompt::WillPrompt) {
        self.prompt = prompt;
    }

    /// Allow the given [`Voice`] to speak using an optional instruction
    /// override.
    pub fn command_voice_to_speak(&self, voice: &crate::voice::Voice, prompt: Option<String>) {
        voice.permit(prompt);
    }
}

#[async_trait]
impl Summarizer<String, String> for Will {
    async fn digest(&self, inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        let input = inputs
            .last()
            .map(|i| i.raw_data.clone())
            .unwrap_or_default();
        let instruction = Instruction {
            command: self.prompt.build(&input),
            images: Vec::new(),
        };
        let resp = self.doer.follow(instruction.clone()).await?;
        let decision = resp.trim().to_string();
        if let Some(tx) = &self.tx {
            let _ = tx.send(crate::WitReport {
                name: "Will".into(),
                prompt: instruction.command.clone(),
                output: decision.clone(),
            });
        }
        Ok(Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: decision.clone(),
            details: None,
            raw_data: decision,
        })
    }
}
