use crate::{Impression, voice::Voice, wit::Wit, wits::Will, Summarizer};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// Wit driving Pete's actions via the [`Will`] summarizer.
///
/// Accumulates awareness statements and periodically decides what to do or
/// say next. After generating a decision it commands the [`Voice`] to speak.
pub struct WillWit {
    will: Will,
    buffer: Mutex<Vec<Impression<String>>>,
    voice: Arc<Voice>,
}

impl WillWit {
    /// Create a new `WillWit` using `will` to decide actions and allowing
    /// `voice` to speak.
    pub fn new(will: Will, voice: Arc<Voice>) -> Self {
        Self {
            will,
            buffer: Mutex::new(Vec::new()),
            voice,
        }
    }
}

#[async_trait]
impl Wit<Impression<String>, String> for WillWit {
    async fn observe(&self, input: Impression<String>) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Option<Impression<String>> {
        let inputs = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return None;
            }
            let data = buf.clone();
            buf.clear();
            data
        };
        let decision = self.will.digest(&inputs).await.ok()?;
        self.will.command_voice_to_speak(&self.voice, None);
        Some(decision)
    }
}
