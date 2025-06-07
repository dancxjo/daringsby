use tokio::sync::mpsc;

use crate::{Result, TTSError};

/// Queue sentences and invoke a speak function sequentially.
pub struct SentenceStreamer {
    tx: mpsc::Sender<String>,
}

impl SentenceStreamer {
    /// Spawn a new background task using the provided synchronous speak function.
    pub fn new<F>(mut speak: F) -> Self
    where
        F: FnMut(String) -> Result<Vec<u8>> + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<String>(8);
        tokio::spawn(async move {
            while let Some(sentence) = rx.recv().await {
                let _ = speak(sentence);
            }
        });
        Self { tx }
    }

    /// Enqueue a sentence to be synthesized.
    pub async fn enqueue(&self, text: String) -> Result<()> {
        self.tx
            .send(text)
            .await
            .map_err(|_| TTSError::QueueClosed)?;
        Ok(())
    }
}
