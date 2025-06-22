use async_trait::async_trait;
use psyche::Ear;
#[cfg(feature = "ear")]
use psyche::{Sensation, Voice};
#[cfg(feature = "ear")]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[cfg(feature = "ear")]
use tokio::sync::mpsc;
#[cfg(feature = "ear")]
use tracing::{debug, info};

#[cfg(feature = "ear")]
/// [`Ear`] implementation that forwards heard text through a channel.
#[derive(Clone)]
pub struct ChannelEar {
    forward: mpsc::Sender<Sensation>,
    speaking: Arc<AtomicBool>,
    voice: Arc<Voice>,
}

#[cfg(feature = "ear")]
impl ChannelEar {
    /// Create a new `ChannelEar` wired to the given channels.
    pub fn new(
        forward: mpsc::Sender<Sensation>,
        speaking: Arc<AtomicBool>,
        voice: Arc<Voice>,
    ) -> Self {
        Self {
            forward,
            speaking,
            voice,
        }
    }

    /// Human readable description of this sense.
    pub const DESCRIPTION: &'static str = "Pete hears audio from the user, transcribed as text. He can respond to spoken questions and converse naturally.";
}

#[cfg(feature = "ear")]
#[async_trait]
impl Ear for ChannelEar {
    async fn hear_self_say(&self, text: &str) {
        self.speaking.store(false, Ordering::SeqCst);
        info!(%text, "ear heard self say");
        debug!("ear heard self say: {}", text);
        self.voice.permit(None);
        let _ = self
            .forward
            .send(Sensation::HeardOwnVoice(text.to_string()))
            .await;
    }

    async fn hear_user_say(&self, text: &str) {
        info!(%text, "ear heard user say");
        debug!("ear heard user say: {}", text);
        let _ = self
            .forward
            .send(Sensation::HeardUserVoice(text.to_string()))
            .await;
    }
}

/// [`Ear`] implementation that ignores all input.
#[derive(Clone)]
pub struct NoopEar;

#[async_trait]
impl Ear for NoopEar {
    async fn hear_self_say(&self, _text: &str) {}
    async fn hear_user_say(&self, _text: &str) {}
}
