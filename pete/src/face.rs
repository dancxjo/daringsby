use psyche::{Countenance, Event};
use tokio::sync::broadcast;

/// [`Countenance`] implementation that forwards emoji updates over a broadcast channel.
#[derive(Clone)]
pub struct ChannelCountenance {
    events: broadcast::Sender<Event>,
}

impl ChannelCountenance {
    /// Create a new `ChannelCountenance` using the given [`broadcast::Sender`].
    pub fn new(events: broadcast::Sender<Event>) -> Self {
        Self { events }
    }
}

impl Countenance for ChannelCountenance {
    fn express(&self, emoji: &str) {
        let _ = self.events.send(Event::EmotionChanged(emoji.to_string()));
    }
}

/// No-op [`Countenance`] used when no feedback channel is available.
#[derive(Clone, Default)]
pub struct NoopFace;

impl Countenance for NoopFace {
    fn express(&self, _emoji: &str) {}
}
