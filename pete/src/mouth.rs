use crate::EventBus;
use async_trait::async_trait;
use lingproc::segment_text_into_sentences;
use psyche::{Event, traits::Mouth};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::{debug, info};

/// Simple mouth implementation that does not produce audio.
///
/// `ChannelMouth` segments text into sentences and dispatches
/// [`Event::Speech`] events without audio for each one while toggling a
/// shared speaking flag.
#[derive(Clone)]
pub struct ChannelMouth {
    bus: Arc<EventBus>,
    speaking: Arc<AtomicBool>,
}

impl ChannelMouth {
    /// Create a new `ChannelMouth` that publishes speech on `bus`.
    pub fn new(bus: Arc<EventBus>, speaking: Arc<AtomicBool>) -> Self {
        Self { bus, speaking }
    }
}

#[async_trait]
impl Mouth for ChannelMouth {
    async fn speak(&self, text: &str) {
        self.speaking.store(true, Ordering::SeqCst);
        info!(%text, "mouth speaking");
        debug!("mouth speaking: {}", text);
        for sentence in segment_text_into_sentences(text) {
            let sent = sentence.trim();
            if !sent.is_empty() {
                self.bus.publish_event(Event::Speech {
                    text: sent.to_string(),
                    audio: None,
                });
            }
        }
        self.speaking.store(false, Ordering::SeqCst);
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
        info!("mouth interrupted");
        debug!("mouth interrupted");
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

/// [`Mouth`] implementation that only toggles a speaking flag.
#[derive(Clone)]
pub struct NoopMouth {
    speaking: Arc<AtomicBool>,
}

impl Default for NoopMouth {
    fn default() -> Self {
        Self {
            speaking: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Mouth for NoopMouth {
    async fn speak(&self, _text: &str) {
        self.speaking.store(true, Ordering::SeqCst);
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}
