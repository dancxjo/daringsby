use crate::Impression;
use async_trait::async_trait;

/// A cognitive unit that distills input into an [`Impression`].
///
/// Wits operate asynchronously and may be chained together to form
/// layered cognition. The `process` method consumes input and returns
/// an impression summarizing it.
#[async_trait]
pub trait Wit<I, O>: Send + Sync {
    /// Process the input and produce an impression.
    async fn process(&self, input: I) -> Impression<O>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[derive(Default)]
    struct EchoWit;

    /// A trivial [`Wit`] used for tests that wraps the input string
    /// into an [`Impression`].
    #[async_trait]
    impl Wit<String, String> for EchoWit {
        async fn process(&self, input: String) -> Impression<String> {
            Impression {
                headline: input.clone(),
                details: None,
                raw_data: input,
            }
        }
    }

    #[tokio::test]
    async fn echo_wit_returns_impression() {
        let wit = EchoWit::default();
        let imp = wit.process("hi".to_string()).await;
        assert_eq!(imp.headline, "hi");
        assert_eq!(imp.raw_data, "hi");
    }
}
