use crate::{Impression, Wit, ling::Doer};
use async_trait::async_trait;
use std::sync::Arc;

/// Decide Pete's next action or speech using a language model.
///
/// `Will` sends the given situation summary to a [`Doer`] with a
/// brief prompt asking for a single sentence describing what Pete
/// should do or say next. The decision is returned as an
/// [`Impression`].
///
/// # Example
/// ```no_run
/// # use psyche::{Will, ling::Doer, Impression, Wit};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Doer for Dummy {
/// #   async fn follow(&self, _s: &str) -> anyhow::Result<String> {
/// #       Ok("Speak.".to_string())
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() {
/// let will = Will::new(Box::new(Dummy));
/// let imp = will
///     .digest(&[Impression { headline: "".into(), details: None, raw_data: "greet the user".to_string() }])
///     .await
///     .unwrap();
/// assert_eq!(imp.raw_data, "Speak.");
/// # }
/// ```
#[derive(Clone)]
pub struct Will {
    doer: Arc<dyn Doer>,
}

impl Will {
    /// Create a new `Will` using the provided [`Doer`].
    pub fn new(doer: Box<dyn Doer>) -> Self {
        Self { doer: doer.into() }
    }
}

#[async_trait]
impl Wit<String, String> for Will {
    async fn digest(&self, inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        let input = inputs
            .last()
            .map(|i| i.raw_data.clone())
            .unwrap_or_default();
        let instruction =
            format!("In one short sentence, what should Pete do or say next?\n{input}");
        let resp = self.doer.follow(&instruction).await?;
        let decision = resp.trim().to_string();
        Ok(Impression {
            headline: decision.clone(),
            details: None,
            raw_data: decision,
        })
    }
}
