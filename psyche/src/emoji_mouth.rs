//! Mouth wrapper that routes emoji characters to a [`Countenance`].
//!
//! `EmojiMouth` strips emoji graphemes from the provided text before forwarding
//! it to the inner [`Mouth`]. Any removed emojis are sent to the associated
//! [`Countenance`] in the order they appear.
//!
//! ```no_run
//! use psyche::{EmojiMouth, Mouth, Countenance};
//! use std::sync::Arc;
//!
//! # struct DummyMouth;
//! # #[async_trait::async_trait]
//! # impl Mouth for DummyMouth {
//! #     async fn speak(&self, _t: &str) {}
//! #     async fn interrupt(&self) {}
//! #     fn speaking(&self) -> bool { false }
//! # }
//! # #[derive(Clone)]
//! # struct DummyFace;
//! # impl Countenance for DummyFace { fn express(&self, _emoji: &str) {} }
//! let mouth = Arc::new(DummyMouth) as Arc<dyn Mouth>;
//! let face = Arc::new(DummyFace) as Arc<dyn Countenance>;
//! let mouth = EmojiMouth::new(mouth, face);
//! ```
use crate::{Countenance, Mouth};
use async_trait::async_trait;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// [`Mouth`] implementation that filters emoji out of spoken text.
#[derive(Clone)]
pub struct EmojiMouth {
    inner: Arc<dyn Mouth>,
    face: Arc<dyn Countenance>,
}

impl EmojiMouth {
    /// Create a new [`EmojiMouth`] wrapping `inner` and updating `face`.
    pub fn new(inner: Arc<dyn Mouth>, face: Arc<dyn Countenance>) -> Self {
        Self { inner, face }
    }
}

#[async_trait]
impl Mouth for EmojiMouth {
    async fn speak(&self, text: &str) {
        let mut plain = String::new();
        let mut emoji = String::new();
        for g in UnicodeSegmentation::graphemes(text, true) {
            if emojis::get(g).is_some() {
                emoji.push_str(g);
            } else {
                plain.push_str(g);
            }
        }
        if !emoji.is_empty() {
            self.face.express(&emoji);
        }
        let trimmed = plain.trim();
        if !trimmed.is_empty() {
            self.inner.speak(trimmed).await;
        }
    }

    async fn interrupt(&self) {
        self.inner.interrupt().await;
    }

    fn speaking(&self) -> bool {
        self.inner.speaking()
    }
}
