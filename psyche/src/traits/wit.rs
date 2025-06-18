use crate::{Impression, Sensation, ling::Instruction};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use std::sync::Arc;
use uuid::Uuid;

/// A Wit is a layer of cognition that summarizes prior impressions into a more
/// abstract one.
///
/// Each Wit listens for input impressions of a lower level and, on tick, emits
/// a higher-level [`Impression`].
#[async_trait]
pub trait Wit<Input, Output>: Send + Sync {
    /// Feed an incoming input (e.g. Sensation or lower-level Impression).
    async fn observe(&self, input: Input);

    /// Periodically called to emit a summarized [`Impression`].
    async fn tick(&self) -> Option<Impression<Output>>;
}

/// Type-erased wrapper enabling heterogeneous [`Wit`]s to be stored together.
#[async_trait]
pub trait ErasedWit: Send + Sync {
    /// Execute a tick and return an [`Impression`] with the payload serialized.
    async fn tick_erased(&self) -> Option<Impression<Value>>;
}

/// Adapter allowing any [`Wit`] to be used as an [`ErasedWit`].
pub struct WitAdapter<I, O> {
    inner: Arc<dyn Wit<I, O> + Send + Sync>,
}

impl<I, O> WitAdapter<I, O> {
    /// Wrap `wit` so it can be stored as an [`ErasedWit`].
    pub fn new(wit: Arc<dyn Wit<I, O> + Send + Sync>) -> Self {
        Self { inner: wit }
    }
}

#[async_trait]
impl<I, O> ErasedWit for WitAdapter<I, O>
where
    O: Serialize + Send + Sync + 'static,
{
    async fn tick_erased(&self) -> Option<Impression<Value>> {
        self.inner.tick().await.and_then(|imp| {
            serde_json::to_value(imp.raw_data)
                .ok()
                .map(|raw| Impression {
                    id: imp.id,
                    timestamp: imp.timestamp,
                    headline: imp.headline,
                    details: imp.details,
                    raw_data: raw,
                })
        })
    }
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
pub struct Situation {
    /// Concise description of the current situation.
    pub summary: String,
}

/// A high level summary of a situation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    /// Narrative recap of recent situations.
    pub summary: String,
}

/// A Wit turning [`Sensation`]s into [`Instant`]s.
pub struct InstantWit {
    doer: Arc<dyn crate::ling::Doer>,
}

impl InstantWit {
    /// Create a new `InstantWit` using the provided [`Doer`].
    pub fn new(doer: Box<dyn crate::ling::Doer>) -> Self {
        Self { doer: doer.into() }
    }
}

impl Default for InstantWit {
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
impl Summarizer<Sensation, Instant> for InstantWit {
    async fn digest(
        &self,
        inputs: &[Impression<Sensation>],
    ) -> anyhow::Result<Impression<Instant>> {
        let mut combined = String::new();
        for imp in inputs {
            if !combined.is_empty() {
                combined.push(' ');
            }
            let desc = match &imp.raw_data {
                Sensation::HeardOwnVoice(t) => format!("Pete said: {t}"),
                Sensation::HeardUserVoice(t) => format!("User said: {t}"),
                Sensation::Of(_) => "Something happened".to_string(),
            };
            combined.push_str(&desc);
        }
        let prompt = format!(
            "Summarize the following sensations in one sentence:\n{}",
            combined
        );
        let resp = self
            .doer
            .follow(Instruction {
                command: prompt,
                images: Vec::new(),
            })
            .await?;
        let observation = resp.trim().to_string();
        Ok(Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: observation.clone(),
            details: Some(combined),
            raw_data: Instant { observation },
        })
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
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
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

impl SituationWit {
    /// Create a new `SituationWit` using the provided [`Doer`].
    pub fn new(doer: Box<dyn crate::ling::Doer>) -> Self {
        Self { doer: doer.into() }
    }
}

impl Default for SituationWit {
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
impl Summarizer<Moment, Situation> for SituationWit {
    async fn digest(&self, inputs: &[Impression<Moment>]) -> anyhow::Result<Impression<Situation>> {
        let mut combined = String::new();
        for imp in inputs {
            if !combined.is_empty() {
                combined.push(' ');
            }
            combined.push_str(&imp.raw_data.summary);
        }
        let prompt = format!(
            "Summarize the following moments in one sentence:\n{}",
            combined
        );
        let resp = self
            .doer
            .follow(Instruction {
                command: prompt,
                images: Vec::new(),
            })
            .await?;
        let summary = resp.trim().to_string();
        Ok(Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: summary.clone(),
            details: Some(combined),
            raw_data: Situation { summary },
        })
    }
}

/// A Wit summarizing [`Situation`]s into an [`Episode`].
pub struct EpisodeWit {
    doer: Arc<dyn crate::ling::Doer>,
}

impl EpisodeWit {
    /// Create a new `EpisodeWit` using the provided [`Doer`].
    pub fn new(doer: Box<dyn crate::ling::Doer>) -> Self {
        Self { doer: doer.into() }
    }
}

impl Default for EpisodeWit {
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
impl Summarizer<Situation, Episode> for EpisodeWit {
    async fn digest(
        &self,
        inputs: &[Impression<Situation>],
    ) -> anyhow::Result<Impression<Episode>> {
        let mut combined = String::new();
        for imp in inputs {
            if !combined.is_empty() {
                combined.push(' ');
            }
            combined.push_str(&imp.raw_data.summary);
        }
        let prompt = format!(
            "Summarize these situations into a short episode:\n{}",
            combined
        );
        let resp = self
            .doer
            .follow(Instruction {
                command: prompt,
                images: Vec::new(),
            })
            .await?;
        let summary = resp.trim().to_string();
        Ok(Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: summary.clone(),
            details: Some(combined),
            raw_data: Episode { summary },
        })
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
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
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
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
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
    impl Wit<String, String> for DummyWit {
        async fn observe(&self, input: String) {
            self.data.lock().unwrap().push(input);
        }

        async fn tick(&self) -> Option<Impression<String>> {
            let mut data = self.data.lock().unwrap();
            let summary = data.join(", ");
            data.clear();
            Some(Impression {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
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
