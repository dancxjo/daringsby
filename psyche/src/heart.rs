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

/// Determine the emotional tone of text using an LLM.
///
/// `Heart` sends the provided text to a [`Doer`] with a prompt asking
/// for an emoji summarizing the emotion. The resulting emoji is wrapped
/// in an [`Impression`].
///
/// # Example
/// ```no_run
/// # use psyche::{Heart, ling::{Doer, Instruction}, Impression, Summarizer};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Doer for Dummy {
/// #   async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
/// #       Ok("😊".to_string())
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() {
/// let heart = Heart::new(Box::new(Dummy));
/// let imp = heart
///     .digest(&[Impression::new("", Some("Great job!"), "".to_string())])
///     .await
///     .unwrap();
/// assert_eq!(imp.raw_data, "😊");
/// # }
/// ```
#[derive(Clone)]
pub struct Heart {
    doer: Arc<dyn Doer>,
    prompt: crate::prompt::HeartPrompt,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl Heart {
    /// Create a new `Heart` using the given [`Doer`].
    pub fn new(doer: Box<dyn Doer>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::HeartPrompt::default(),
            tx: None,
        }
    }

    /// Create a `Heart` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(doer: Box<dyn Doer>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::HeartPrompt::default(),
            tx: Some(tx),
        }
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: crate::prompt::HeartPrompt) {
        self.prompt = prompt;
    }
}

#[async_trait]
impl Summarizer<String, String> for Heart {
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
        let emoji = resp.trim().to_string();
        if let Some(tx) = &self.tx {
            let _ = tx.send(crate::WitReport {
                name: "Heart".into(),
                prompt: instruction.command.clone(),
                output: emoji.clone(),
            });
        }
        Ok(Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: emoji.clone(),
            details: None,
            raw_data: emoji,
        })
    }
}
