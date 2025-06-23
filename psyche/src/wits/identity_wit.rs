use crate::{
    Impression, Summarizer,
    wit::{Moment, Wit},
    wits::FondDuCoeur,
};
use async_trait::async_trait;
use std::sync::Mutex;

/// Wit that produces a single-paragraph life story from recent moments.
pub struct IdentityWit {
    summarizer: FondDuCoeur,
    buffer: Mutex<Vec<Impression<Moment>>>,
}

impl IdentityWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "IdentityWit";
    /// Create a new `IdentityWit` using the given summarizer.
    pub fn new(summarizer: FondDuCoeur) -> Self {
        Self {
            summarizer,
            buffer: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Wit for IdentityWit {
    type Input = Impression<Moment>;
    type Output = String;

    async fn observe(&self, input: Self::Input) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
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
