use crate::{
    Impression, Wit,
    ling::{Chatter, Message, Role},
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio_stream::StreamExt;

/// Decide Pete's next action or speech using a language model.
///
/// `Will` sends the given situation summary to a [`Chatter`] with a
/// brief prompt asking for a single sentence describing what Pete
/// should do or say next. The decision is returned as an
/// [`Impression`].
///
/// # Example
/// ```no_run
/// # use psyche::{Will, ling::{Chatter, Message}, Impression, Wit};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Chatter for Dummy {
/// #   async fn chat(&self, _p: &str, _h: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
/// #       Ok(Box::pin(tokio_stream::once(Ok("Speak.".to_string()))))
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() {
/// let will = Will::new(Box::new(Dummy));
/// let imp = will.process("greet the user".to_string()).await;
/// assert_eq!(imp.raw_data, "Speak.");
/// # }
/// ```
#[derive(Clone)]
pub struct Will {
    chatter: Arc<dyn Chatter>,
}

impl Will {
    /// Create a new `Will` using the provided [`Chatter`].
    pub fn new(chatter: Box<dyn Chatter>) -> Self {
        Self {
            chatter: chatter.into(),
        }
    }
}

#[async_trait]
impl Wit<String, String> for Will {
    async fn process(&self, input: String) -> Impression<String> {
        let prompt = "In one short sentence, what should Pete do or say next?";
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
        let decision = resp.trim().to_string();
        Impression {
            headline: decision.clone(),
            details: None,
            raw_data: decision,
        }
    }
}
