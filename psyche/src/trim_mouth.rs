//! Mouth wrapper that trims leading and trailing whitespace.
//!
//! `TrimMouth` forwards speech to an inner [`Mouth`] only if the
//! trimmed text is not empty. This helps avoid speaking stray
//! whitespace emitted by language models.
//!
//! ```no_run
//! use psyche::{TrimMouth, Mouth};
//! use std::sync::Arc;
//!
//! # struct Dummy;
//! # #[async_trait::async_trait]
//! # impl Mouth for Dummy {
//! #     async fn speak(&self, _t: &str) {}
//! #     async fn interrupt(&self) {}
//! #     fn speaking(&self) -> bool { false }
//! # }
//! let inner = Arc::new(Dummy) as Arc<dyn Mouth>;
//! let mouth = TrimMouth::new(inner);
//! mouth.speak("  hello  ");
//! ```
use crate::Mouth;
use async_trait::async_trait;
use std::sync::Arc;

/// [`Mouth`] implementation that trims text before speaking.
#[derive(Clone)]
pub struct TrimMouth {
    inner: Arc<dyn Mouth>,
}

impl TrimMouth {
    /// Create a new [`TrimMouth`] wrapping `inner`.
    pub fn new(inner: Arc<dyn Mouth>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl Mouth for TrimMouth {
    async fn speak(&self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        self.inner.speak(trimmed).await;
    }

    async fn interrupt(&self) {
        self.inner.interrupt().await;
    }

    fn speaking(&self) -> bool {
        self.inner.speaking()
    }
}
