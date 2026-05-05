use async_trait::async_trait;
use psyche::traits::Ear;
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
use tracing::{debug, info, warn};

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
    pub const DESCRIPTION: &'static str = "You hear audio from the user, transcribed as text. He can respond to spoken questions and converse naturally.";

    fn queue_sensation(&self, sensation: Sensation, label: &'static str) {
        let forward = self.forward.clone();
        tokio::spawn(async move {
            if forward.send(sensation).await.is_err() {
                warn!(
                    label,
                    "failed to queue heard speech; psyche input is closed"
                );
            }
        });
    }
}

#[cfg(feature = "ear")]
#[async_trait]
impl Ear for ChannelEar {
    async fn hear_self_say(&self, text: &str) {
        self.speaking.store(false, Ordering::SeqCst);
        info!(%text, "ear heard self say");
        debug!("ear heard self say: {}", text);
        self.voice.permit(None);
        self.queue_sensation(Sensation::HeardOwnVoice(text.to_string()), "self");
    }

    async fn hear_user_say(&self, text: &str) {
        info!(%text, "ear heard user say");
        debug!("ear heard user say: {}", text);
        self.voice.permit(None);
        self.queue_sensation(Sensation::HeardUserVoice(text.to_string()), "user");
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
