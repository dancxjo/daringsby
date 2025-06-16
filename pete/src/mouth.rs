use async_trait::async_trait;
use psyche::{Event, Mouth};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::broadcast;
use tracing::debug;

#[derive(Clone)]
pub struct ChannelMouth {
    events: broadcast::Sender<Event>,
    speaking: Arc<AtomicBool>,
}

impl ChannelMouth {
    pub fn new(events: broadcast::Sender<Event>, speaking: Arc<AtomicBool>) -> Self {
        Self { events, speaking }
    }
}

#[async_trait]
impl Mouth for ChannelMouth {
    async fn speak(&self, text: &str) {
        self.speaking.store(true, Ordering::SeqCst);
        debug!("mouth speaking: {}", text);
        let seg = pragmatic_segmenter::Segmenter::new().expect("segmenter init");
        for sentence in seg.segment(text) {
            let sent = sentence.trim();
            if !sent.is_empty() {
                let _ = self.events.send(Event::IntentionToSay(sent.to_string()));
            }
        }
        self.speaking.store(false, Ordering::SeqCst);
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
        debug!("mouth interrupted");
        let _ = self.events.send(Event::IntentionToSay(String::new()));
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

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
