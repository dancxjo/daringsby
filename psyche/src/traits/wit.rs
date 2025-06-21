use crate::{Impression, Sensation, Stimulus, ling::Instruction};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use std::sync::Arc;

/// A Wit is a layer of cognition that summarizes prior impressions into a more
/// abstract one.
///
/// Each Wit listens for input impressions of a lower level and, on tick, emits
/// a higher-level [`Impression`].
#[async_trait]
pub trait Wit<Input, Output>: Send + Sync {
    /// Feed an incoming input (e.g. Sensation or lower-level Impression).
    async fn observe(&self, input: Input);

    /// Periodically called to emit zero or more summarized [`Impression`]s.
    async fn tick(&self) -> Vec<Impression<Output>>;

    /// Human readable name used for logging.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Short static label used for debug filters.
    fn debug_label(&self) -> &'static str {
        self.name()
    }
}

/// Type-erased wrapper enabling heterogeneous [`Wit`]s to be stored together.
#[async_trait]
pub trait ErasedWit: Send + Sync {
    /// Execute a tick and return serialized [`Impression`]s.
    async fn tick_erased(&self) -> Vec<Impression<Value>>;

    /// Name of this [`Wit`]. Used for debugging.
    fn name(&self) -> &'static str;

    /// Debug label of this [`Wit`].
    fn debug_label(&self) -> &'static str;
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
    async fn tick_erased(&self) -> Vec<Impression<Value>> {
        self.inner
            .tick()
            .await
            .into_iter()
            .map(|imp| {
                let stimuli = imp
                    .stimuli
                    .into_iter()
                    .filter_map(|s| {
                        serde_json::to_value(&s.what).ok().map(|what| Stimulus {
                            what,
                            timestamp: s.timestamp,
                        })
                    })
                    .collect();
                Impression {
                    stimuli,
                    summary: imp.summary,
                    emoji: imp.emoji,
                    timestamp: imp.timestamp,
                }
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn debug_label(&self) -> &'static str {
        self.inner.debug_label()
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
            if let Some(stim) = imp.stimuli.first() {
                if !combined.is_empty() {
                    combined.push(' ');
                }
                let desc = match &stim.what {
                    Sensation::HeardOwnVoice(t) => format!("Pete said: {t}"),
                    Sensation::HeardUserVoice(t) => format!("User said: {t}"),
                    Sensation::Of(_) => "Something happened".to_string(),
                };
                combined.push_str(&desc);
            }
        }
        let prompt = format!(
            "Summarize the following sensations in one or two sentences:\n{}",
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
        let stim = Stimulus::new(Instant {
            observation: observation.clone(),
        });
        Ok(Impression::new(vec![stim], observation, None::<String>))
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
            if let Some(stim) = imp.stimuli.first() {
                if !combined.is_empty() {
                    combined.push(' ');
                }
                combined.push_str(&stim.what.observation);
            }
        }
        let combined = combined.trim().to_string();

        let prompt = format!(
            "Summarize the following observations in one or two sentences:\n{}",
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

        Ok(Impression::new(
            vec![Stimulus::new(Moment {
                summary: summary.clone(),
            })],
            summary,
            None::<String>,
        ))
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
            if let Some(stim) = imp.stimuli.first() {
                if !combined.is_empty() {
                    combined.push(' ');
                }
                combined.push_str(&stim.what.summary);
            }
        }
        let prompt = format!(
            "Summarize the following moments in one or two sentences:\n{}",
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
        Ok(Impression::new(
            vec![Stimulus::new(Situation {
                summary: summary.clone(),
            })],
            summary,
            None::<String>,
        ))
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
            if let Some(stim) = imp.stimuli.first() {
                if !combined.is_empty() {
                    combined.push(' ');
                }
                combined.push_str(&stim.what.summary);
            }
        }
        let prompt = format!(
            "Summarize these situations in one or two sentences:\n{}",
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
        Ok(Impression::new(
            vec![Stimulus::new(Episode {
                summary: summary.clone(),
            })],
            summary,
            None::<String>,
        ))
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
                .and_then(|i| i.stimuli.first())
                .map(|s| s.what.clone())
                .unwrap_or_default();
            Ok(Impression::new(
                vec![Stimulus::new(input.clone())],
                input,
                None::<String>,
            ))
        }
    }

    #[tokio::test]
    async fn echo_wit_returns_impression() {
        let wit = EchoWit::default();
        let imp = wit
            .digest(&[Impression::new(
                vec![Stimulus::new("hi".to_string())],
                "hi",
                None::<String>,
            )])
            .await
            .unwrap();
        assert_eq!(imp.summary, "hi");
        assert_eq!(imp.stimuli[0].what, "hi");
    }

    struct DummyWit {
        data: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Wit<String, String> for DummyWit {
        async fn observe(&self, input: String) {
            self.data.lock().unwrap().push(input);
        }

        async fn tick(&self) -> Vec<Impression<String>> {
            let mut data = self.data.lock().unwrap();
            if data.is_empty() {
                return Vec::new();
            }
            let summary = data.join(", ");
            data.clear();
            vec![Impression::new(
                vec![Stimulus::new("dummy".to_string())],
                format!("Summarized {} items", summary.split(", ").count()),
                None::<String>,
            )]
        }
    }

    #[tokio::test]
    async fn it_summarizes_input_on_tick() {
        let wit = DummyWit {
            data: Mutex::new(Vec::new()),
        };
        wit.observe("foo".to_string()).await;
        wit.observe("bar".to_string()).await;
        let result = wit.tick().await;
        assert_eq!(result.len(), 1);
        let result = &result[0];
        assert!(result.summary.contains("Summarized 2"));
    }
}
