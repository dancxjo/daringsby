use crate::{Impression, Summarizer, voice::Voice, wit::Wit, wits::Will};
use async_trait::async_trait;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

/// Wit driving Pete's actions via the [`Will`] summarizer.
///
/// Accumulates awareness statements and periodically decides what to do or
/// say next. After generating a decision it commands the [`Voice`] to speak.
pub struct WillWit {
    will: Arc<Will>,
    buffer: Mutex<Vec<Impression<String>>>,
    voice: Arc<Voice>,
    ticks: AtomicUsize,
}

impl WillWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "WillWit";
    /// Create a new `WillWit` using `will` to decide actions and allowing
    /// `voice` to speak.
    pub fn new(will: Arc<Will>, voice: Arc<Voice>) -> Self {
        voice.set_will(will.clone());
        Self {
            will,
            buffer: Mutex::new(Vec::new()),
            voice,
            ticks: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Wit<Impression<String>, String> for WillWit {
    async fn observe(&self, input: Impression<String>) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<String>> {
        let inputs = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            let data = buf.clone();
            buf.clear();
            data
        };
        let decision = match self.will.digest(&inputs).await {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };

        let count = self.ticks.fetch_add(1, Ordering::SeqCst) + 1;
        let mut prompt = None;
        if count % 3 == 0 {
            prompt = Some("share a brief update".to_string());
        }
        if inputs.iter().any(|i| i.raw_data.contains('?')) {
            prompt = Some("answer the user's question".to_string());
        }
        if let Some(p) = prompt {
            self.voice.permit(Some(p));
        }

        self.will.command_voice_to_speak(&self.voice, None);
        vec![decision]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
