use crate::{
    Impression, Summarizer,
    wit::{Moment, Wit},
    wits::FondDuCoeur,
};
use async_trait::async_trait;
use std::sync::Mutex;

/// Wit that produces a single-paragraph life story from recent moments.
pub struct FondDuCoeurWit {
    summarizer: FondDuCoeur,
    buffer: Mutex<Vec<Impression<Moment>>>,
}

impl FondDuCoeurWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "FondDuCoeurWit";
    /// Create a new `FondDuCoeurWit` using the given summarizer.
    pub fn new(summarizer: FondDuCoeur) -> Self {
        Self {
            summarizer,
            buffer: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Wit<Impression<Moment>, String> for FondDuCoeurWit {
    async fn observe(&self, input: Impression<Moment>) {
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
        match self.summarizer.digest(&inputs).await {
            Ok(i) => vec![i],
            Err(_) => Vec::new(),
        }
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
