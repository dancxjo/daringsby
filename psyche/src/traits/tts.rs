use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Stream of raw WAV data chunks.
pub type TtsStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>;

/// Text-to-speech engine interface.
#[async_trait]
pub trait Tts: Send + Sync {
    /// Return a stream of WAV bytes for `text`.
    async fn stream_wav(&self, text: &str) -> Result<TtsStream>;
}
