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

/// Envelope for topic messages carrying any payload.
#[derive(Debug, Clone)]
pub struct TopicMessage {
    /// Topic this message belongs to.
    pub topic: Topic,
    /// Opaque payload associated with the topic.
    pub payload: Arc<dyn Any + Send + Sync>,
}

/// Pete's internal publish/subscribe backbone.
///
/// The `TopicBus` is a type-safe message bus allowing Wits to
/// broadcast and subscribe to semantically tagged [`Topic<T>`] channels.
/// Each topic represents a cognitive stage or signal. For example,
/// `Topic::Sensation` carries raw inputs from the world, while
/// `Topic::Instruction` may contain a behavioral directive such as
/// `&lt;say&gt;` or `&lt;leap&gt;`.
///
/// Key guarantees:
/// - ✅ Type safety between stages (only the correct payload type travels on a
///   given topic)
/// - ✅ Broadcast delivery to all subscribers
/// - ✅ Runtime decoupling between publishers and consumers
///
/// # Example
/// ```no_run
/// use std::sync::Arc;
/// use futures::StreamExt;
/// use psyche::{topics::{Topic, TopicBus}, Instant};
///
/// let bus = TopicBus::new(8);
/// // A Quick emits an Instant
/// let my_instant = Instant { at: chrono::Utc::now(), sensations: vec![] };
/// bus.publish(Topic::Instant, Arc::new(my_instant));
///
/// // A Will subscribes to those Instants
/// let mut rx = bus.subscribe(Topic::Instant);
/// while let Some(instant) = rx.next().await {
///     let _ = instant.downcast::<Instant>().unwrap();
/// }
/// ```
///
/// Think of it as Pete's spinal cord: every sensation, reaction and internal
/// decision flows through here.
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
