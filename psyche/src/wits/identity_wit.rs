use crate::traits::BufferedWit;
use crate::{Impression, WitReport, wit::Wit, wits::FondDuCoeur};
use async_trait::async_trait;
use std::sync::Mutex;
use tokio::sync::broadcast;

/// Wit that produces a single-paragraph life story from recent moments.
pub struct IdentityWit {
    summarizer: FondDuCoeur,
    buffer: Mutex<Vec<Impression<String>>>,
    tx: Option<broadcast::Sender<WitReport>>,
}

impl IdentityWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "IdentityWit";
    /// Create a new `IdentityWit` using the given summarizer.
    pub fn new(summarizer: FondDuCoeur) -> Self {
        Self::with_debug(summarizer, None)
    }

    /// Create an `IdentityWit` that emits [`WitReport`]s via `tx`.
    pub fn with_debug(summarizer: FondDuCoeur, tx: Option<broadcast::Sender<WitReport>>) -> Self {
        Self {
            summarizer,
            buffer: Mutex::new(Vec::new()),
            tx,
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
            Ok(i) => {
                if let Some(tx) = &self.tx {
                    if crate::debug::debug_enabled(Self::LABEL).await {
                        let _ = tx.send(WitReport {
                            name: Self::LABEL.into(),
                            prompt: "identity digest".into(),
                            output: i.summary.clone(),
                        });
                    }
                }
                vec![i]
            }
            Err(_) => Vec::new(),
        }
    }

    fn label(&self) -> &'static str {
        Self::LABEL
    }
}
