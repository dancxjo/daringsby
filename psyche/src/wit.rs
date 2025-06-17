use crate::{Impression, Sensation};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A Wit is a layer of cognition that summarizes prior impressions into a more
/// abstract one.
///
/// Each Wit listens for input impressions of a lower level and, on tick, emits
/// a higher-level [`Impression`].
#[async_trait]
pub trait Wit<Input>: Send + Sync {
    /// Feed an incoming input (e.g. Sensation or lower-level Impression).
    async fn observe(&self, input: Input);

    /// Periodically called to emit a summarized [`Impression`].
    async fn tick(&self) -> Option<Impression<Input>>;
}

/// A cognitive unit that distills input into an [`Impression`].
///
/// Wits operate asynchronously and may be chained together to form
/// layered cognition. The `digest` method consumes a batch of lower level
/// impressions and produces a higher level [`Impression`].
#[async_trait]
pub trait Summarizer<I, O>: Send + Sync {
    /// Digest `inputs` into a single summarizing [`Impression`].
    async fn digest(&self, inputs: &[Impression<I>]) -> anyhow::Result<Impression<O>>;
}

/// A raw observation in time.
/// A raw observation in time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instant {
    /// Description of the sensed event.
    pub observation: String,
}

/// A short sequence of instants summarized in text.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Moment {
    /// Brief textual summary of the moment.
    pub summary: String,
}

/// An aggregation of moments providing more context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Situation;

/// A high level summary of a situation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode;

/// A Wit turning [`Sensation`]s into [`Instant`]s.
pub struct InstantWit {
    doer: Arc<dyn crate::ling::Doer>,
}

#[async_trait]
impl Summarizer<Sensation, Instant> for InstantWit {
    async fn digest(
        &self,
        _inputs: &[Impression<Sensation>],
    ) -> anyhow::Result<Impression<Instant>> {
        todo!()
    }
}

/// A Wit summarizing [`Instant`]s into a [`Moment`].
#[derive(Clone)]
pub struct MomentWit {
    doer: Arc<dyn crate::ling::Doer>,
}

impl MomentWit {
    /// Create a new `MomentWit` using the provided [`Doer`].
    pub fn new(doer: Box<dyn crate::ling::Doer>) -> Self {
        Self { doer: doer.into() }
    }
}

impl Default for MomentWit {
    fn default() -> Self {
        #[derive(Clone)]
        struct Dummy;

        #[async_trait]
        impl crate::ling::Doer for Dummy {
            async fn follow(
                &self,
                instruction: crate::ling::Instruction,
            ) -> anyhow::Result<String> {
                Ok(instruction.command)
            }
        }

        Self::new(Box::new(Dummy))
    }
}

#[async_trait]
impl Summarizer<Instant, Moment> for MomentWit {
    async fn digest(&self, inputs: &[Impression<Instant>]) -> anyhow::Result<Impression<Moment>> {
        // Join headlines, details and observations into one paragraph.
        let mut combined = String::new();
        for imp in inputs {
            if !combined.is_empty() {
                combined.push(' ');
            }
            if !imp.headline.is_empty() {
                combined.push_str(&imp.headline);
                combined.push(' ');
            }
            if let Some(details) = &imp.details {
                combined.push_str(details);
                combined.push(' ');
            }
            combined.push_str(&imp.raw_data.observation);
        }
        let combined = combined.trim().to_string();

        let prompt = format!(
            "Summarize the following observations into one short paragraph:\n{}",
            combined
        );

        // For now we simply echo the prompt as the model response.
        let resp = self
            .doer
            .follow(crate::ling::Instruction {
                command: prompt,
                images: Vec::new(),
            })
            .await?;
        let summary = resp.trim().to_string();

        Ok(Impression {
            headline: summary.clone(),
            details: Some(combined),
            raw_data: Moment { summary },
        })
    }
}

/// A Wit distilling [`Moment`]s into a [`Situation`].
pub struct SituationWit {
    doer: Arc<dyn crate::ling::Doer>,
}

#[async_trait]
impl Summarizer<Moment, Situation> for SituationWit {
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
impl Summarizer<Situation, Episode> for EpisodeWit {
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
    use std::sync::Mutex;

    #[derive(Default)]
    struct EchoWit;

    /// A trivial [`Summarizer`] used for tests that wraps the last input string
    /// into an [`Impression`].
    #[async_trait]
    impl Summarizer<String, String> for EchoWit {
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

    struct DummyWit {
        data: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Wit<String> for DummyWit {
        async fn observe(&self, input: String) {
            self.data.lock().unwrap().push(input);
        }

        async fn tick(&self) -> Option<Impression<String>> {
            let mut data = self.data.lock().unwrap();
            let summary = data.join(", ");
            data.clear();
            Some(Impression {
                headline: format!("Summarized {} items", summary.split(", ").count()),
                details: Some(summary),
                raw_data: "dummy".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn it_summarizes_input_on_tick() {
        let wit = DummyWit {
            data: Mutex::new(Vec::new()),
        };
        wit.observe("foo".to_string()).await;
        wit.observe("bar".to_string()).await;
        let result = wit.tick().await.unwrap();
        assert!(result.headline.contains("Summarized 2"));
        assert_eq!(result.details.unwrap(), "foo, bar");
    }
}
