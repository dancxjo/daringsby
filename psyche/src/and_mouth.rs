//! Mouth combinator that fans out speech to multiple mouths.
//!
//! Create an [`AndMouth`] with any number of mouth implementations and it will
//! forward all [`Mouth`] calls to each one. This is useful for scenarios where
//! you want text-to-speech audio and textual display at the same time.
//!
//! ```no_run
//! use psyche::{AndMouth, Mouth};
//! use std::sync::Arc;
//!
//! # struct Dummy;
//! # #[async_trait::async_trait]
//! # impl Mouth for Dummy {
//! #     async fn speak(&self, _t: &str) {}
//! #     async fn interrupt(&self) {}
//! #     fn speaking(&self) -> bool { false }
//! # }
//!
//! let mouths: Vec<Arc<dyn Mouth>> = vec![Arc::new(Dummy), Arc::new(Dummy)];
//! let mouth = AndMouth::new(mouths);
//! ```
//!
use crate::Mouth;
use async_trait::async_trait;
use futures::future::{BoxFuture, join_all};
use std::sync::Arc;

/// [`Mouth`] implementation that broadcasts calls to several other mouths.
#[derive(Clone, Default)]
pub struct AndMouth {
    mouths: Vec<Arc<dyn Mouth>>,
}

impl AndMouth {
    /// Create a new [`AndMouth`] from any iterator of mouths.
    pub fn new<I>(mouths: I) -> Self
    where
        I: IntoIterator<Item = Arc<dyn Mouth>>,
    {
        Self {
            mouths: mouths.into_iter().collect(),
        }
    }

    fn for_each_async<'a, F>(&'a self, mut f: F) -> BoxFuture<'a, ()>
    where
        F: FnMut(&'a Arc<dyn Mouth>) -> BoxFuture<'a, ()> + Send + 'a,
    {
        Box::pin(async move {
            let futures = self.mouths.iter().map(|m| f(m));
            join_all(futures).await;
        })
    }
}

#[async_trait]
impl Mouth for AndMouth {
    async fn speak(&self, text: &str) {
        self.for_each_async(|m| Box::pin(m.speak(text))).await;
    }

    async fn interrupt(&self) {
        self.for_each_async(|m| Box::pin(m.interrupt())).await;
    }

    fn speaking(&self) -> bool {
        self.mouths.iter().any(|m| m.speaking())
    }
}
