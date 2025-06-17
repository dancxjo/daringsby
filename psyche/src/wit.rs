use crate::{Impression, Sensation};
use async_trait::async_trait;
use std::sync::Arc;

/// A cognitive unit that distills input into an [`Impression`].
///
/// Wits operate asynchronously and may be chained together to form
/// layered cognition. The `digest` method consumes a batch of lower level
/// impressions and produces a higher level [`Impression`].
#[async_trait]
pub trait Wit<I, O>: Send + Sync {
    /// Digest `inputs` into a single summarizing [`Impression`].
    async fn digest(&self, inputs: &[Impression<I>]) -> anyhow::Result<Impression<O>>;
}

/// A raw observation in time.
#[derive(Clone, Debug)]
pub struct Instant;

/// A short sequence of instants.
#[derive(Clone, Debug)]
pub struct Moment;

/// An aggregation of moments providing more context.
#[derive(Clone, Debug)]
pub struct Situation;

/// A high level summary of a situation.
#[derive(Clone, Debug)]
pub struct Episode;

/// A Wit turning [`Sensation`]s into [`Instant`]s.
pub struct InstantWit {
    doer: Arc<dyn crate::ling::Doer>,
}

#[async_trait]
impl Wit<Sensation, Instant> for InstantWit {
    async fn digest(
        &self,
        _inputs: &[Impression<Sensation>],
    ) -> anyhow::Result<Impression<Instant>> {
        todo!()
    }
}

/// A Wit summarizing [`Instant`]s into a [`Moment`].
pub struct MomentWit {
    doer: Arc<dyn crate::ling::Doer>,
}

#[async_trait]
impl Wit<Instant, Moment> for MomentWit {
    async fn digest(&self, _inputs: &[Impression<Instant>]) -> anyhow::Result<Impression<Moment>> {
        todo!()
    }
}

/// A Wit distilling [`Moment`]s into a [`Situation`].
pub struct SituationWit {
    doer: Arc<dyn crate::ling::Doer>,
}

#[async_trait]
impl Wit<Moment, Situation> for SituationWit {
    async fn digest(
        &self,
        _inputs: &[Impression<Moment>],
    ) -> anyhow::Result<Impression<Situation>> {
        todo!()
    }
}

/// A Wit summarizing [`Situation`]s into an [`Episode`].
pub struct EpisodeWit {
    doer: Arc<dyn crate::ling::Doer>,
}

#[async_trait]
impl Wit<Situation, Episode> for EpisodeWit {
    async fn digest(
        &self,
        _inputs: &[Impression<Situation>],
    ) -> anyhow::Result<Impression<Episode>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[derive(Default)]
    struct EchoWit;

    /// A trivial [`Wit`] used for tests that wraps the last input string
    /// into an [`Impression`].
    #[async_trait]
    impl Wit<String, String> for EchoWit {
        async fn digest(
            &self,
            inputs: &[Impression<String>],
        ) -> anyhow::Result<Impression<String>> {
            let input = inputs
                .last()
                .map(|i| i.raw_data.clone())
                .unwrap_or_default();
            Ok(Impression {
                headline: input.clone(),
                details: None,
                raw_data: input,
            })
        }
    }

    #[tokio::test]
    async fn echo_wit_returns_impression() {
        let wit = EchoWit::default();
        let imp = wit
            .digest(&[Impression {
                headline: "hi".into(),
                details: None,
                raw_data: "hi".to_string(),
            }])
            .await
            .unwrap();
        assert_eq!(imp.headline, "hi");
        assert_eq!(imp.raw_data, "hi");
    }
}
