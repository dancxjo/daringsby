use crate::{
    Impression, Wit,
    ling::{Chatter, Message, Role},
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio_stream::StreamExt;

/// Determine the emotional tone of text using an LLM.
///
/// `Heart` sends the provided text to a [`Chatter`] with a prompt asking
/// for an emoji summarizing the emotion. The resulting emoji is wrapped
/// in an [`Impression`].
///
/// # Example
/// ```no_run
/// # use psyche::{Heart, ling::{Chatter, Message}, Impression, Wit};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Chatter for Dummy {
/// #   async fn chat(&self, _s: &str, _h: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
/// #       Ok(Box::pin(tokio_stream::once(Ok("😊".to_string()))))
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() {
/// let heart = Heart::new(Box::new(Dummy));
/// let imp = heart.process("Great job!".to_string()).await;
/// assert_eq!(imp.raw_data, "😊");
/// # }
/// ```
#[derive(Clone)]
pub struct Heart {
    chatter: Arc<dyn Chatter>,
}

impl Heart {
    /// Create a new `Heart` using the given [`Chatter`].
    pub fn new(chatter: Box<dyn Chatter>) -> Self {
        Self {
            chatter: chatter.into(),
        }
    }
}

#[async_trait]
impl Wit<String, String> for Heart {
    async fn process(&self, input: String) -> Impression<String> {
        let prompt = "Respond with a single emoji describing the overall emotion";
        let history = [Message {
            role: Role::User,
            content: input.clone(),
        }];
        let mut stream = self
            .chatter
            .chat(prompt, &history)
            .await
            .unwrap_or_else(|_| Box::pin(tokio_stream::empty()));
        let mut resp = String::new();
        while let Some(chunk) = stream.next().await.transpose().unwrap_or_default() {
            resp.push_str(&chunk);
        }
        let emoji = resp.trim().to_string();
        Impression {
            headline: emoji.clone(),
            details: None,
            raw_data: emoji,
        }
    }
}
