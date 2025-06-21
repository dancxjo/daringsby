use crate::{
    Impression, Summarizer,
    wit::{Episode, Wit},
    wits::Combobulator,
};
use async_trait::async_trait;
use std::sync::Mutex;

/// Wit summarizing recent episodes into a short awareness statement.
pub struct CombobulatorWit {
    combobulator: Combobulator,
    buffer: Mutex<Vec<Impression<Episode>>>,
}

impl CombobulatorWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "CombobulatorWit";
    /// Create a new `CombobulatorWit` using the given summarizer.
    pub fn new(combobulator: Combobulator) -> Self {
        Self {
            combobulator,
            buffer: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Wit<Impression<Episode>, String> for CombobulatorWit {
    async fn observe(&self, input: Impression<Episode>) {
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
        match self.combobulator.digest(&inputs).await {
            Ok(i) => vec![i],
            Err(_) => Vec::new(),
        }
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}
