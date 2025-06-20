use crate::{
    Impression,
    wit::{Episode, Wit},
    wits::Combobulator,
    Summarizer,
};
use async_trait::async_trait;
use std::sync::Mutex;

/// Wit summarizing recent episodes into a short awareness statement.
pub struct CombobulatorWit {
    combobulator: Combobulator,
    buffer: Mutex<Vec<Impression<Episode>>>,
}

impl CombobulatorWit {
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
        self.combobulator.digest(&inputs).await.ok()
    }
}
