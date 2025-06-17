use crate::{Impression, Wit, ling::Doer};
use async_trait::async_trait;
use std::sync::Arc;

/// Determine the emotional tone of text using an LLM.
///
/// `Heart` sends the provided text to a [`Doer`] with a prompt asking
/// for an emoji summarizing the emotion. The resulting emoji is wrapped
/// in an [`Impression`].
///
/// # Example
/// ```no_run
/// # use psyche::{Heart, ling::Doer, Impression, Wit};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Doer for Dummy {
/// #   async fn follow(&self, _s: &str) -> anyhow::Result<String> {
/// #       Ok("😊".to_string())
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() {
/// let heart = Heart::new(Box::new(Dummy));
/// let imp = heart
///     .digest(&[Impression { headline: "".into(), details: None, raw_data: "Great job!".to_string() }])
///     .await
///     .unwrap();
/// assert_eq!(imp.raw_data, "😊");
/// # }
/// ```
#[derive(Clone)]
pub struct Heart {
    doer: Arc<dyn Doer>,
}

impl Heart {
    /// Create a new `Heart` using the given [`Doer`].
    pub fn new(doer: Box<dyn Doer>) -> Self {
        Self { doer: doer.into() }
    }
}

#[async_trait]
impl Wit<String, String> for Heart {
    async fn digest(&self, inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        let input = inputs
            .last()
            .map(|i| i.raw_data.clone())
            .unwrap_or_default();
        let instruction =
            format!("Respond with a single emoji describing the overall emotion of:\n{input}");
        let resp = self.doer.follow(&instruction).await?;
        let emoji = resp.trim().to_string();
        Ok(Impression {
            headline: emoji.clone(),
            details: None,
            raw_data: emoji,
        })
    }
}
