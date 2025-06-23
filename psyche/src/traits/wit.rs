use crate::{Impression, Stimulus};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{self, Value};
use std::sync::Arc;

/// A Wit is a layer of cognition that summarizes prior impressions into a more
/// abstract one.
///
/// Each Wit listens for input impressions of a lower level and, on tick, emits
/// a higher-level [`Impression`].
#[async_trait]
pub trait Wit: Send + Sync {
    /// Type of input observed by this Wit.
    type Input: Send;
    /// Type of output produced by this Wit.
    type Output: Send;

    /// Feed an incoming input (e.g. Sensation or lower-level Impression).
    async fn observe(&self, input: Self::Input);

    /// Periodically called to emit zero or more summarized [`Impression`]s.
    async fn tick(&self) -> Vec<Impression<Self::Output>>;

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
pub struct WitAdapter<W: Wit> {
    inner: Arc<W>,
}

impl<W: Wit> WitAdapter<W> {
    /// Wrap `wit` so it can be stored as an [`ErasedWit`].
    pub fn new(wit: Arc<W>) -> Self {
        Self { inner: wit }
    }
}

#[async_trait]
impl<W> ErasedWit for WitAdapter<W>
where
    W: Wit,
    W::Output: Serialize + Send + Sync + 'static,
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct DummyWit {
        data: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Wit for DummyWit {
        type Input = String;
        type Output = String;

        async fn observe(&self, input: Self::Input) {
            self.data.lock().unwrap().push(input);
        }

        async fn tick(&self) -> Vec<Impression<Self::Output>> {
            let mut data = self.data.lock().unwrap();
            if data.is_empty() {
                return Vec::new();
            }
            let summary = data.join(", ");
            data.clear();
            vec![Impression::new(
                vec![crate::Stimulus::new("dummy".to_string())],
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
