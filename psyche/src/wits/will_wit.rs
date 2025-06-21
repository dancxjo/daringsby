use crate::{Impression, Stimulus, Summarizer, voice::Voice, wit::Wit, wits::Will};
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

    /// Override the [`Voice`] prompt builder.
    pub fn set_prompt<P>(&self, prompt: P)
    where
        P: crate::prompt::PromptBuilder + Send + Sync + 'static,
    {
        self.voice.set_prompt(prompt);
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
        if inputs
            .iter()
            .any(|i| i.stimuli.iter().any(|s| s.what.contains('?')))
        {
            prompt = Some("answer the user's question".to_string());
        }

        let mut out = vec![decision];
        if let Some(p) = prompt {
            out.push(Impression::new(
                vec![Stimulus::new(format!("<take_turn>{}</take_turn>", p))],
                "take_turn",
                None::<String>,
            ));
        }
        out
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
