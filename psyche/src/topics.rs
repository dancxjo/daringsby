use futures::Stream;
use futures::StreamExt;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

/// Cognitive topics exchanged between Wits.
///
/// These define the semantic channels used for communication within the
/// `psyche`. Each variant represents a stage in Pete's cognitive stack,
/// following the arc from raw sensation to high-level identity.
///
/// Topics are implemented as typed channels using [`Topic<T>`] and consumed via
/// the [`TopicBus`] for decoupled publish/subscribe communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic {
    /// 🧿 Raw sensory input from the environment.
    ///
    /// Emitted by sensors like audio transcribers, webcam processors,
    /// geolocation, or heartbeat. Received by the Quick and other low-level
    /// perception components.
    Sensation,

    /// ⚡️ Immediate perceptual awareness.
    ///
    /// Generated by the Quick by summarizing a batch of [`Sensation`]s into a
    /// single coherent [`Instant`]. Represents Pete’s immediate experience
    /// (“I see a fly approaching”).
    Instant,

    /// 🧭 Integrated summary of nearby events.
    ///
    /// Emitted by the Combobulator by collecting several [`Instant`]s into a
    /// cohesive [`Moment`]. Useful for recognizing short patterns or behavior
    /// changes (“The fly hovered near me for a few seconds”).
    Moment,

    /// 🧩 Contextualized narrative.
    ///
    /// A broader frame that gathers multiple [`Moment`]s into a structured
    /// [`Situation`]. Useful for forming a behavioral stance (“There is prey
    /// nearby, and I am preparing to strike”).
    Situation,

    /// 🧠 Episodic memory trace.
    ///
    /// Constructed by Memory using linked [`Impression`]s. Represents a durable
    /// event in Pete’s timeline.
    /// (“Earlier today, I saw a spider stalk a fly, then retreat.”)
    Episode,

    /// 🪞 Narrative self-image.
    ///
    /// Maintained by Identity (FondDuCoeur). A paragraph-long summary of
    /// Pete’s self-perception, role, and goals.
    /// Regularly updated as Pete changes or reflects.
    Identity,

    /// 🛠️ Intent to act.
    ///
    /// Emitted by the Will. Contains an LLM-parsed imperative like `<say>` or
    /// `<leap>`, along with semantic parameters.
    /// Interpreted by the Voice, Motor, or other effectors.
    Instruction,

    /// 🧬 Facial recognition metadata.
    ///
    /// Encodes face vectors or named face matches seen in camera input.
    /// Shared with Memory and Vision for grounding social identity.
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

/// Pete's internal publish/subscribe backbone.
///
/// The `TopicBus` is a type-safe message bus allowing Wits to
/// broadcast and subscribe to semantically tagged [`Topic<T>`] channels.
/// Each topic represents a cognitive stage or signal. For example,
/// `Topic::Sensation` carries raw inputs from the world, while
/// `Topic::Instruction` may contain a behavioral directive such as
/// `<say>` or `<leap>`.
///
/// Key guarantees:
/// - ✅ Type safety between stages (only the correct payload type travels on a
///   given topic)
/// - ✅ Broadcast delivery to all subscribers
/// - ✅ Runtime decoupling between publishers and consumers
///
/// # Example
/// ```rust,ignore
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
