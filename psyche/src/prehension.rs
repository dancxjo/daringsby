use crate::{
    Impression,
    wit::{Summarizer, Wit},
};
use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::Mutex;
use tracing::debug;

/// Collects impressions in a buffer and periodically summarizes them.
///
/// `Prehension` forms the glue between multiple [`Wit`] stages. It buffers
/// incoming impressions and, on [`tick`], uses a [`Summarizer`] to produce a
/// higher-level impression.
///
/// # Example
/// ```
/// use psyche::{Prehension, wit::Summarizer};
/// use psyche::Impression;
/// use async_trait::async_trait;
///
/// struct Echo;
/// #[async_trait]
/// impl Summarizer<String, String> for Echo {
///     async fn digest(
///         &self,
///         inputs: &[Impression<String>],
///     ) -> anyhow::Result<Impression<String>> {
///         let joined = inputs.iter().map(|i| i.raw_data.clone()).collect::<Vec<_>>().join(" ");
///         Ok(Impression::new(joined.clone(), None::<String>, joined))
///     }
/// }
/// let wit = Prehension::new(Echo);
/// ```
pub struct Prehension<I, O, S> {
    summarizer: S,
    buffer: Mutex<Vec<Impression<I>>>,
    _marker: PhantomData<O>,
}

impl<I, O, S> Prehension<I, O, S>
where
    S: Summarizer<I, O> + Send + Sync,
{
    /// Create a new `Prehension` using the given [`Summarizer`].
    pub fn new(summarizer: S) -> Self {
        Self {
            summarizer,
            buffer: Mutex::new(Vec::new()),
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<I, O, S> Wit<Impression<I>, O> for Prehension<I, O, S>
where
    S: Summarizer<I, O> + Send + Sync,
    I: Send + Sync + Clone + 'static,
    O: Send + Sync + 'static,
{
    async fn observe(&self, input: Impression<I>) {
        debug!("prehension observed input");
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Option<Impression<O>> {
        let inputs = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return None;
            }
            let data = buf.clone();
            buf.clear();
            data
        };
        debug!("prehension digesting {} items", inputs.len());
        self.summarizer.digest(&inputs).await.ok()
    }
}
