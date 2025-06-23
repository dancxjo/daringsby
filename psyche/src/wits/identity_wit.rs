use crate::traits::BufferedWit;
use crate::{Impression, wits::FondDuCoeur};
use async_trait::async_trait;
use std::sync::Mutex;

/// Wit that produces a single-paragraph life story from recent moments.
pub struct IdentityWit {
    summarizer: FondDuCoeur,
    buffer: Mutex<Vec<Impression<String>>>,
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
impl BufferedWit for IdentityWit {
    type Input = Impression<String>;
    type Output = String;

    fn buffer(&self) -> &Mutex<Vec<Self::Input>> {
        &self.buffer
    }

    async fn process_buffer(&self, inputs: Vec<Self::Input>) -> Vec<Impression<Self::Output>> {
        match self.summarizer.digest(&inputs).await {
            Ok(i) => vec![i],
            Err(_) => Vec::new(),
        }
    }

    fn label(&self) -> &'static str {
        Self::LABEL
    }
}
