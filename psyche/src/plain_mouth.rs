//! Mouth wrapper that removes Markdown formatting.
//!
//! `PlainMouth` forwards text stripped of Markdown markup to an inner [`Mouth`].
//! This ensures the TTS engine receives clean plain text.
//!
//! ```no_run
//! use psyche::{PlainMouth, Mouth};
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
//! let mouth = PlainMouth::new(inner);
//! mouth.speak("**hi**");
//! ```
use crate::Mouth;
use async_trait::async_trait;
use pulldown_cmark::{Event, Parser};
use std::sync::Arc;

/// [`Mouth`] implementation that strips Markdown before speaking.
#[derive(Clone)]
pub struct PlainMouth {
    inner: Arc<dyn Mouth>,
}

impl PlainMouth {
    /// Create a new [`PlainMouth`] wrapping `inner`.
    pub fn new(inner: Arc<dyn Mouth>) -> Self {
        Self { inner }
    }

    fn strip(text: &str) -> String {
        let mut out = String::new();
        for event in Parser::new(text) {
            match event {
                Event::Text(t) | Event::Code(t) => out.push_str(&t),
                Event::SoftBreak | Event::HardBreak => out.push(' '),
                _ => {}
            }
        }
        out.replace(['*', '_', '`'], "")
    }
}

#[async_trait]
impl Mouth for PlainMouth {
    async fn speak(&self, text: &str) {
        let plain = Self::strip(text);
        let trimmed = plain.trim();
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
