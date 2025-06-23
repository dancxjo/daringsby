use futures::Stream;
use futures::StreamExt;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

/// Cognitive topics exchanged between Wits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic {
    Sensation,
    Instant,
    Moment,
    Situation,
    Episode,
    Identity,
    Instruction,
    FaceInfo,
}

/// Envelope for messages exchanged on the [`TopicBus`].
///
/// A `TopicMessage` is usually created by [`TopicBus::publish`] and then
/// received via [`TopicBus::subscribe_raw`].  The [`payload`] field contains the
/// published value boxed as `Arc<dyn Any + Send + Sync>` so callers must
/// downcast it to the expected type.
///
/// # Example
/// ```
/// use psyche::topics::{TopicBus, Topic};
///
/// # #[tokio::main]
/// # async fn main() {
/// let bus = TopicBus::new(8);
/// bus.publish(Topic::Sensation, "hi".to_string());
/// let mut rx = bus.subscribe_raw();
/// let msg = rx.recv().await.unwrap();
/// assert_eq!(msg.topic, Topic::Sensation);
/// let text = msg.payload.downcast_ref::<String>().unwrap();
/// assert_eq!(text, "hi");
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TopicMessage {
    /// Topic this message belongs to.
    pub topic: Topic,
    /// Opaque payload associated with the topic.
    pub payload: Arc<dyn Any + Send + Sync>,
}

/// Simple async pub/sub bus tagged by [`Topic`].
#[derive(Clone)]
pub struct TopicBus {
    tx: broadcast::Sender<TopicMessage>,
}

impl TopicBus {
    /// Create a new bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _r) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish `payload` on `topic` to all subscribers.
    pub fn publish(&self, topic: Topic, payload: impl Any + Send + Sync + 'static) {
        let _ = self.tx.send(TopicMessage {
            topic,
            payload: Arc::new(payload),
        });
    }

    /// Subscribe to messages tagged with `topic`.
    pub fn subscribe(&self, topic: Topic) -> impl Stream<Item = Arc<dyn Any + Send + Sync>> {
        BroadcastStream::new(self.tx.subscribe()).filter_map(move |res| {
            let topic = topic;
            async move {
                match res {
                    Ok(msg) if msg.topic == topic => Some(msg.payload),
                    _ => None,
                }
            }
        })
    }

    /// Subscribe to all raw messages.
    pub fn subscribe_raw(&self) -> broadcast::Receiver<TopicMessage> {
        self.tx.subscribe()
    }
}
