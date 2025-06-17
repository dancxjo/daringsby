use async_trait::async_trait;
use psyche::{Ear, Sensation};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Mutex, mpsc};
use tracing::debug;

/// [`Ear`] implementation that forwards heard text through a channel.
#[derive(Clone)]
pub struct ChannelEar {
    forward: mpsc::UnboundedSender<Sensation>,
    conversation: Arc<Mutex<psyche::Conversation>>, // share log from psyche
    speaking: Arc<AtomicBool>,
}

impl ChannelEar {
    /// Create a new `ChannelEar` wired to the given channels.
    pub fn new(
        forward: mpsc::UnboundedSender<Sensation>,
        conversation: Arc<Mutex<psyche::Conversation>>,
        speaking: Arc<AtomicBool>,
    ) -> Self {
        Self {
            forward,
            conversation,
            speaking,
        }
    }
}

#[async_trait]
impl Ear for ChannelEar {
    async fn hear_self_say(&self, text: &str) {
        self.speaking.store(false, Ordering::SeqCst);
        debug!("ear heard self say: {}", text);
        let _ = self
            .forward
            .send(Sensation::HeardOwnVoice(text.to_string()));
    }

    async fn hear_user_say(&self, text: &str) {
        debug!("ear heard user say: {}", text);
        let _ = self
            .forward
            .send(Sensation::HeardUserVoice(text.to_string()));
        let mut conv = self.conversation.lock().await;
        conv.add_user(text.to_string());
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
