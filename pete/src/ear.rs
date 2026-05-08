use async_trait::async_trait;
#[cfg(feature = "ear")]
use chrono::{DateTime, Utc};
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
use tracing::{info, trace, warn};

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
        self.hear_self_say_at(text, Utc::now()).await;
    }

    async fn hear_self_say_at(&self, text: &str, occurred_at: DateTime<Utc>) {
        self.speaking.store(false, Ordering::SeqCst);
        info!(%text, "ear heard self say");
        self.voice.permit(None);
        self.queue_sensation(
            Sensation::heard_own_voice_at(text.to_string(), occurred_at),
            "self",
        );
    }

    async fn hear_user_say(&self, text: &str) {
        self.hear_user_say_at(text, Utc::now()).await;
    }

    async fn hear_user_say_at(&self, text: &str, occurred_at: DateTime<Utc>) {
        info!(%text, "ear heard user say");
        trace!("ear heard user say queued");
        self.voice.permit(None);
        self.queue_sensation(
            Sensation::heard_user_voice_at(text.to_string(), occurred_at),
            "user",
        );
    }

    async fn hear_web_interface_type(&self, text: &str) {
        self.hear_web_interface_type_at(text, Utc::now()).await;
    }

    async fn hear_web_interface_type_at(&self, text: &str, occurred_at: DateTime<Utc>) {
        info!(%text, "ear heard web interface text");
        trace!("ear heard web interface text queued");
        self.voice.permit(None);
        self.queue_sensation(
            Sensation::web_interface_text_at(text.to_string(), occurred_at),
            "web interface",
        );
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
