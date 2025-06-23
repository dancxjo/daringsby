use crate::default_prompt::DEFAULT_SYSTEM_PROMPT;
use crate::sensation::{Event, Sensation, WitReport};
use crate::traits::Doer;
use crate::traits::wit;
use crate::traits::wit::{ErasedWit, Wit};
use crate::traits::{Ear, Mouth};
use crate::wits::memory::Memory;
use lingproc::{Chatter, Message, Role, Vectorizer};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

#[cfg(not(test))]
const DEFAULT_EXPERIENCE_TICK: Duration = Duration::from_secs(60);
#[cfg(test)]
const DEFAULT_EXPERIENCE_TICK: Duration = Duration::from_millis(10);
#[cfg(not(test))]
const DEFAULT_ACTIVE_EXPERIENCE_TICK: Duration = Duration::from_secs(5);
#[cfg(test)]
const DEFAULT_ACTIVE_EXPERIENCE_TICK: Duration = Duration::from_millis(5);
use crate::pending_turn::PendingTurn;
/// Default size for internal broadcast channels.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 16;

use crate::task_group::TaskGroup;
use chrono::{DateTime, Utc};
use futures::FutureExt;
use quick_xml::{Reader, events::Event as XmlEvent};
use rand::Rng;
use std::any::Any;
use std::panic::AssertUnwindSafe;
use tokio::sync::{Mutex, broadcast, mpsc};
use tracing::{debug, error, info, warn};

/// A minimal history of exchanged messages.
///
/// `Conversation` collects messages in order so they can be fed back to the language model for context.
#[derive(Default, Clone)]
pub struct Conversation {
    log: Vec<Message>,
}

impl Conversation {
    /// Append a user message to the log, merging with the previous user entry when possible.
    pub fn add_message_from_user(&mut self, content: String) {
        self.append_or_new(Role::User, content);
    }

    /// Append an AI generated message to the log, merging consecutive assistant entries.
    pub fn add_message_from_ai(&mut self, content: String) {
        self.append_or_new(Role::Assistant, content);
    }

    fn append_or_new(&mut self, role: Role, content: String) {
        if let Some(last) = self.log.last_mut() {
            if last.role == role {
                if !last.content.is_empty() && !content.is_empty() {
                    last.content.push(' ');
                }
                last.content.push_str(&content);
                last.content = last.content.trim().to_string();
                return;
            }
        }
        self.log.push(Message { role, content });
    }

    /// Return the last `n` messages from the conversation.
    ///
    /// [`PromptBuilder`](crate::PromptBuilder) calls this when assembling a prompt for the
    /// [`Chatter`](crate::ling::Chatter) so that only recent dialogue is
    /// forwarded. Trimming history keeps model prompts a manageable size.
    pub fn tail(&self, n: usize) -> Vec<Message> {
        let len = self.log.len();
        self.log[len.saturating_sub(n)..].to_vec()
    }

    /// Return the entire conversation history.
    pub fn all(&self) -> &[Message] {
        &self.log
    }
}

#[derive(Debug, Clone, Copy)]
enum SpeakPolicy {
    /// Pete can respond whenever a turn is pending.
    Always,
    /// Pete only responds after hearing a user message.
    /// The flag tracks whether such a message has been received.
    WhenSpokenTo { user_message_pending: bool },
}

impl SpeakPolicy {
    fn waiting_for_user(&self) -> bool {
        matches!(
            self,
            SpeakPolicy::WhenSpokenTo {
                user_message_pending: false
            }
        )
    }

    fn received_user_message(&mut self) {
        if let SpeakPolicy::WhenSpokenTo {
            user_message_pending,
        } = self
        {
            *user_message_pending = true;
        }
    }

    fn after_speech(&mut self) {
        if let SpeakPolicy::WhenSpokenTo {
            user_message_pending,
        } = self
        {
            *user_message_pending = false;
        }
    }
}

/// The core AI engine coordinating conversation.
///
/// `Psyche` drives interactions with language models and orchestrates IO via the [`Mouth`] and [`Ear`] traits. Instantiate it and call [`Psyche::run`] to start the loop.
pub struct Psyche {
    #[allow(dead_code)]
    narrator: Box<dyn Doer>,
    voice: Arc<crate::voice::Voice>,
    #[allow(dead_code)]
    vectorizer: Box<dyn Vectorizer>,
    memory: Arc<dyn Memory>,
    ear: Arc<dyn Ear>,
    emotion: String,
    system_prompt: String,
    max_history: usize,
    max_turns: usize,
    events_tx: broadcast::Sender<Event>,
    input_tx: mpsc::Sender<Sensation>,
    input_rx: mpsc::Receiver<Sensation>,
    conversation: Arc<Mutex<Conversation>>,
    echo_timeout: Duration,
    /// Delay between experience ticks.
    experience_tick: Duration,
    /// Faster tick rate during active conversation.
    active_experience_tick: Duration,
    is_speaking: Arc<AtomicBool>,
    speak_policy: SpeakPolicy,
    connections: Option<Arc<AtomicUsize>>,
    wits: Vec<Arc<dyn wit::ErasedWit + Send + Sync>>,
    wit_tx: broadcast::Sender<WitReport>,
    prompt_builder: Arc<Mutex<crate::PromptBuilder>>,
    observers: Vec<Arc<dyn crate::traits::observer::SensationObserver + Send + Sync>>,
    sensation_buffer: Arc<Mutex<VecDeque<Arc<Sensation>>>>,
    last_ticks: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    pending_turn: Arc<PendingTurn>,
    topic_bus: crate::topics::TopicBus,
    fallback_turn: bool,
}

#[doc(hidden)]
pub fn extract_tag(text: &str, name: &str) -> Option<String> {
    let mut reader = Reader::from_str(text);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut inside = false;
    let mut content = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) if e.name().as_ref() == name.as_bytes() => {
                inside = true;
            }
            Ok(XmlEvent::End(e)) if e.name().as_ref() == name.as_bytes() => {
                break;
            }
            Ok(XmlEvent::Text(t)) if inside => {
                content.push_str(&t.unescape().unwrap_or_default());
            }
            Ok(XmlEvent::Eof) => break,
            Err(e) => {
                warn!(?e, "XML parsing failed; falling back to substring search");
                return fallback_extract(text, name);
            }
            _ => {}
        }
        buf.clear();
    }
    if inside && !content.is_empty() {
        Some(content)
    } else {
        debug!("fallback extracting tag" = %name);
        fallback_extract(text, name)
    }
}

fn fallback_extract(text: &str, name: &str) -> Option<String> {
    let start_tag = format!("<{}>", name);
    let end_tag = format!("</{}>", name);
    let start = text.find(&start_tag)? + start_tag.len();
    let end = text[start..].find(&end_tag)? + start;
    Some(text[start..end].to_string())
}

impl Psyche {
    /// Construct a new [`Psyche`] using the given language model providers and IO components.
    pub fn new(
        narrator: Box<dyn Doer>,
        voice: Box<dyn Chatter>,
        vectorizer: Box<dyn Vectorizer>,
        memory: Arc<dyn Memory>,
        mouth: Arc<dyn Mouth>,
        ear: Arc<dyn Ear>,
    ) -> Self {
        Self::with_channel_capacity(
            narrator,
            voice,
            vectorizer,
            memory,
            mouth,
            ear,
            DEFAULT_CHANNEL_CAPACITY,
        )
    }

    /// Construct a [`Psyche`] with custom broadcast channel capacity.
    pub fn with_channel_capacity(
        narrator: Box<dyn Doer>,
        voice: Box<dyn Chatter>,
        vectorizer: Box<dyn Vectorizer>,
        memory: Arc<dyn Memory>,
        mouth: Arc<dyn Mouth>,
        ear: Arc<dyn Ear>,
        capacity: usize,
    ) -> Self {
        let (events_tx, _r) = broadcast::channel(capacity);
        let (wit_tx, _r2) = broadcast::channel(capacity);
        let (input_tx, input_rx) = mpsc::channel(capacity);
        let voice = crate::voice::Voice::new(Arc::from(voice), mouth, events_tx.clone());
        let conversation = Arc::new(Mutex::new(Conversation::default()));
        let prompt_builder = Arc::new(Mutex::new(crate::PromptBuilder::new(
            DEFAULT_SYSTEM_PROMPT,
            conversation.clone(),
        )));
        let pending_turn = Arc::new(PendingTurn::default());
        Self {
            narrator,
            voice: Arc::new(voice),
            vectorizer,
            memory,
            ear,
            emotion: "😐".to_string(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            max_history: 8,
            max_turns: 1,
            events_tx,
            wit_tx,
            input_tx,
            input_rx,
            conversation,
            echo_timeout: Duration::from_secs(1),
            experience_tick: DEFAULT_EXPERIENCE_TICK,
            active_experience_tick: DEFAULT_ACTIVE_EXPERIENCE_TICK,
            is_speaking: Arc::new(AtomicBool::new(false)),
            speak_policy: SpeakPolicy::Always,
            connections: None,
            wits: Vec::new(),
            prompt_builder,
            observers: Vec::new(),
            sensation_buffer: Arc::new(Mutex::new(VecDeque::<Arc<Sensation>>::new())),
            last_ticks: Arc::new(Mutex::new(HashMap::new())),
            pending_turn,
            topic_bus: crate::topics::TopicBus::new(capacity),
            fallback_turn: true,
        }
    }

    /// Specify the base instructions provided to the language model.
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = prompt.into();
        if let Ok(mut pb) = self.prompt_builder.try_lock() {
            pb.set_system_prompt(self.system_prompt.clone());
        }
    }

    /// Retrieve the system prompt currently in use.
    pub fn system_prompt(&self) -> String {
        self.prompt_builder
            .try_lock()
            .map(|l| l.system_prompt().to_string())
            .unwrap_or_default()
    }

    /// Build the system prompt with descriptions of Pete's body.
    pub fn described_system_prompt(&self) -> String {
        self.prompt_builder
            .try_lock()
            .map(|l| l.described_system_prompt())
            .unwrap_or_default()
    }

    /// Limit the number of conversation turns to `turns`.
    pub fn set_turn_limit(&mut self, turns: usize) {
        self.max_turns = turns;
    }

    /// Set how long to wait for the mouth to echo spoken text.
    pub fn set_echo_timeout(&mut self, dur: Duration) {
        self.echo_timeout = dur;
    }

    /// Adjust the delay between experience ticks.
    pub fn set_experience_tick(&mut self, dur: Duration) {
        self.experience_tick = dur;
    }

    /// Adjust the delay for active experience ticks.
    pub fn set_active_experience_tick(&mut self, dur: Duration) {
        self.active_experience_tick = dur;
    }

    /// Retrieve the current experience tick duration.
    pub fn experience_tick(&self) -> Duration {
        self.experience_tick
    }

    /// Retrieve the current active experience tick duration.
    pub fn active_experience_tick(&self) -> Duration {
        self.active_experience_tick
    }

    /// Attach an atomic counter tracking active WebSocket connections.
    pub fn set_connection_counter(&mut self, counter: Arc<AtomicUsize>) {
        self.connections = Some(counter);
    }

    /// Create a new receiver for conversation [`Event`]s.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events_tx.subscribe()
    }

    /// Obtain a sender for queuing [`Sensation`]s to the conversation loop.
    pub fn input_sender(&self) -> mpsc::Sender<Sensation> {
        self.input_tx.clone()
    }

    /// Publish a raw sensory impression on the [`TopicBus`].
    pub fn feel(&self, payload: impl Any + Send + Sync + 'static) {
        self.topic_bus
            .publish(crate::topics::Topic::Sensation, payload);
    }

    /// Obtain the sender used to broadcast conversation [`Event`]s.
    pub fn event_sender(&self) -> broadcast::Sender<Event> {
        self.events_tx.clone()
    }

    /// Update the additional context appended to the system prompt.
    pub async fn update_prompt_context(&self, context: impl Into<String>) {
        self.prompt_builder
            .lock()
            .await
            .add_context_note(&context.into());
    }

    /// Record a description of an attached sense.
    pub fn add_sense(&mut self, description: String) {
        if let Ok(mut pb) = self.prompt_builder.try_lock() {
            pb.add_sense(description);
        }
    }

    /// Obtain the sender used to broadcast [`WitReport`]s.
    pub fn wit_sender(&self) -> broadcast::Sender<WitReport> {
        self.wit_tx.clone()
    }

    /// Subscribe to debugging reports from [`Wit`]s.
    pub fn wit_reports(&self) -> broadcast::Receiver<WitReport> {
        self.wit_tx.subscribe()
    }

    /// Obtain a handle for reading debug info.
    pub fn debug_handle(&self) -> crate::debug::DebugHandle {
        crate::debug::DebugHandle {
            buffer: self.sensation_buffer.clone(),
            ticks: self.last_ticks.clone(),
            wits: self
                .wits
                .iter()
                .map(|w| w.debug_label().to_string())
                .collect(),
        }
    }

    /// Enable debugging for all registered Wits.
    pub async fn enable_all_debug(&self) {
        for label in self.wits.iter().map(|w| w.debug_label()) {
            crate::debug::enable_debug(label).await;
        }
    }

    /// Get a handle to the voice component.
    pub fn voice(&self) -> Arc<crate::voice::Voice> {
        self.voice.clone()
    }

    /// Swap out the [`Mouth`] used for speech output.
    pub fn set_mouth(&mut self, mouth: Arc<dyn Mouth>) {
        self.voice.set_mouth(mouth);
    }

    /// Swap out the [`Memory`] implementation.
    pub fn set_memory(&mut self, memory: Arc<dyn Memory>) {
        self.memory = memory;
    }

    /// Set the currently expressed emotion to `emoji`.
    pub fn set_emotion(&mut self, emoji: impl Into<String>) {
        self.emotion = emoji.into();
        let _ = self
            .events_tx
            .send(Event::EmotionChanged(self.emotion.clone()));
    }

    /// Register a background [`Wit`].
    ///
    /// Example:
    /// ```ignore
    /// use async_trait::async_trait;
    /// use psyche::wit::Wit;
    /// # let mut psyche: psyche::Psyche = todo!();
    /// struct MyWit;
    /// # #[async_trait]
    /// # impl Wit<(), ()> for MyWit {
    /// #   async fn observe(&self, _: ()) {}
    /// #   async fn tick(&self) -> Vec<psyche::Impression<()>> { Vec::new() }
    /// # }
    /// let wit = std::sync::Arc::new(MyWit);
    /// psyche.register_typed_wit(wit);
    /// ```
    pub fn register_wit(&mut self, wit: Arc<dyn ErasedWit + Send + Sync>) {
        self.wits.push(wit);
    }

    /// Convenience to register a typed [`Wit`] without manual boxing.
    pub fn register_typed_wit<W>(&mut self, wit: Arc<W>)
    where
        W: Wit + Send + Sync + 'static,
        W::Output: Serialize + Send + Sync + 'static,
        W::Input: 'static,
    {
        self.wits
            .push(Arc::new(wit::WitAdapter::new(wit)) as Arc<dyn ErasedWit + Send + Sync>);
    }

    /// Register a component that listens for [`Sensation`]s.
    pub fn register_observer(
        &mut self,
        obs: Arc<dyn crate::traits::observer::SensationObserver + Send + Sync>,
    ) {
        self.observers.push(obs);
    }

    /// Register a [`Wit`] that also observes [`Sensation`]s.
    pub fn register_observing_wit<W>(&mut self, wit: Arc<W>)
    where
        W: Wit + crate::traits::observer::SensationObserver + Send + Sync + 'static,
        W::Output: Serialize + Send + Sync + 'static,
        W::Input: 'static,
    {
        self.register_observer(wit.clone());
        self.wits
            .push(Arc::new(wit::WitAdapter::new(wit)) as Arc<dyn ErasedWit + Send + Sync>);
    }

    fn still_conversing(&self, turns: usize) -> bool {
        turns < self.max_turns
    }

    /// Get a handle to the shared conversation history.
    pub fn conversation(&self) -> Arc<Mutex<Conversation>> {
        self.conversation.clone()
    }

    /// Access the shared [`TopicBus`].
    pub fn topic_bus(&self) -> crate::topics::TopicBus {
        self.topic_bus.clone()
    }

    /// Returns `true` if speech has been dispatched but not yet echoed.
    pub fn speaking(&self) -> bool {
        self.is_speaking.load(Ordering::SeqCst)
    }

    /// Enable or disable waiting for user input before speaking.
    pub fn set_speak_when_spoken_to(&mut self, enabled: bool) {
        self.speak_policy = if enabled {
            SpeakPolicy::WhenSpokenTo {
                user_message_pending: false,
            }
        } else {
            SpeakPolicy::Always
        };
    }

    /// Returns `true` if the psyche waits for user messages before speaking.
    pub fn speak_when_spoken_to(&self) -> bool {
        matches!(self.speak_policy, SpeakPolicy::WhenSpokenTo { .. })
    }

    /// Enable or disable the default fallback turn when no Wit sets a turn.
    pub fn set_fallback_turn_enabled(&mut self, enabled: bool) {
        self.fallback_turn = enabled;
    }

    /// Returns `true` if the fallback turn is enabled.
    pub fn fallback_turn_enabled(&self) -> bool {
        self.fallback_turn
    }

    /// Buffer that Pete heard himself say `text`.
    async fn buffer_self_speech(&self, text: &str) {
        self.sensation_buffer
            .lock()
            .await
            .push_back(Arc::new(Sensation::HeardOwnVoice(text.to_string())));
    }

    /// Buffer that the user said `text`.
    async fn buffer_user_speech(&self, text: &str) {
        self.sensation_buffer
            .lock()
            .await
            .push_back(Arc::new(Sensation::HeardUserVoice(text.to_string())));
    }

    async fn notify_observers(&self, sensation: &Sensation) {
        for obs in &self.observers {
            obs.observe_sensation(sensation as &(dyn Any + Send + Sync))
                .await;
        }
    }

    /// Main loop that handles the conversation with the assistant.
    async fn converse(mut self) -> Self {
        info!("psyche conversation started");
        let mut turns = 0;
        while self.still_conversing(turns) {
            if let Some(counter) = &self.connections {
                while counter.load(Ordering::SeqCst) == 0 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
            while let Ok(s) = self.input_rx.try_recv() {
                let arc = Arc::new(s);
                match &*arc {
                    Sensation::HeardOwnVoice(msg) => {
                        let mut conv = self.conversation.lock().await;
                        conv.add_message_from_ai(msg.clone());
                        self.buffer_self_speech(msg).await;
                    }
                    Sensation::HeardUserVoice(msg) => {
                        let mut conv = self.conversation.lock().await;
                        conv.add_message_from_user(msg.clone());
                        self.buffer_user_speech(msg).await;
                        if self.pending_turn.is_empty() && self.fallback_turn {
                            self.pending_turn.set("I'm listening.".to_string());
                        }
                    }
                    Sensation::Of(_) => {
                        self.sensation_buffer.lock().await.push_back(arc.clone());
                    }
                }
                self.notify_observers(arc.as_ref()).await;
            }
            if self.speak_policy.waiting_for_user() {
                match self.input_rx.recv().await {
                    Some(Sensation::HeardUserVoice(msg)) => {
                        debug!("heard user voice: {}", msg);
                        self.ear.hear_user_say(&msg).await;
                        self.buffer_user_speech(&msg).await;
                        if self.pending_turn.is_empty() && self.fallback_turn {
                            self.pending_turn.set("I'm listening.".to_string());
                        }
                        self.notify_observers(&Sensation::HeardUserVoice(msg.clone()))
                            .await;
                        self.speak_policy.received_user_message();
                        continue;
                    }
                    Some(Sensation::HeardOwnVoice(msg)) => {
                        debug!("Received HeardOwnVoice: '{}'", msg);
                        self.ear.hear_self_say(&msg).await;
                        self.buffer_self_speech(&msg).await;
                        self.notify_observers(&Sensation::HeardOwnVoice(msg.clone()))
                            .await;
                        continue;
                    }
                    Some(s @ Sensation::Of(_)) => {
                        debug!("received non-voice sensation while waiting");
                        self.notify_observers(&s).await;
                        self.sensation_buffer.lock().await.push_back(Arc::new(s));
                        continue;
                    }
                    None => break,
                }
            }

            if let Some(extra) = self.pending_turn.take() {
                debug!(%extra, "pending_turn being processed");
                let (history, mut prompt) = {
                    let mut pb = self.prompt_builder.lock().await;
                    let hist = pb.get_conversation_tail(self.max_history).await;
                    let prompt = pb.build_prompt().await;
                    (hist, prompt)
                };
                prompt.push('\n');
                prompt.push_str(&extra);
                info!(%prompt, "conversation prompt");
                self.is_speaking.store(true, Ordering::SeqCst);
                if let Err(e) = self.voice.take_turn(&prompt, &history).await {
                    error!(?e, "voice chat failed");
                    break;
                }
                self.prompt_builder.lock().await.flush();
                debug!("prompt context flushed");
                self.is_speaking.store(false, Ordering::SeqCst);
                self.speak_policy.after_speech();
                turns += 1;
                debug!("turn {} complete", turns);
            } else {
                debug!("no pending_turn available this tick");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        info!("psyche conversation ended");
        self
    }

    /// Background task processing non-conversational experience.
    async fn experience(
        buffer: Arc<Mutex<VecDeque<Arc<Sensation>>>>,
        observers: Vec<Arc<dyn crate::traits::observer::SensationObserver + Send + Sync>>,
        bus: crate::topics::TopicBus,
        idle_tick: Duration,
        active_tick: Duration,
        speaking: Arc<AtomicBool>,
    ) {
        loop {
            let batch: Vec<Arc<Sensation>> = buffer.lock().await.drain(..).collect();
            for s in &batch {
                for obs in &observers {
                    obs.observe_sensation(s.as_ref() as &(dyn Any + Send + Sync))
                        .await;
                }
                bus.publish(crate::topics::Topic::Sensation, s.clone());
            }
            if !batch.is_empty() {
                let instant = crate::sensation::Instant {
                    at: Utc::now(),
                    sensations: batch.clone(),
                };
                bus.publish(crate::topics::Topic::Instant, Arc::new(instant));
                debug!("Published Instant with {} sensations", batch.len());
            }
            let jitter = rand::thread_rng().gen_range(0..50);
            let tick = if speaking.load(Ordering::SeqCst) || !batch.is_empty() {
                active_tick
            } else {
                idle_tick
            };
            tokio::time::sleep(tick + Duration::from_millis(jitter)).await;
        }
    }

    /// Continuous tick loop for a single [`Wit`].
    async fn wit_loop(
        wit: Arc<dyn wit::ErasedWit + Send + Sync>,
        ticks: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
        mem: Arc<dyn Memory>,
        prompt_builder: Arc<Mutex<crate::PromptBuilder>>,
        pending_turn: Arc<PendingTurn>,
        tick: Duration,
    ) {
        loop {
            let name = wit.name();
            debug!(%name, "tick start");
            let imps = wit.tick_erased().await;
            debug!(%name, count = imps.len(), "tick finished");
            let now = Utc::now();
            {
                let mut map = ticks.lock().await;
                map.insert(name.to_string(), now);
            }
            for imp in &imps {
                for stim in &imp.stimuli {
                    if let serde_json::Value::String(s) = &stim.what {
                        if let Some(p) = extract_tag(s, "take_turn") {
                            pending_turn.set(p);
                        }
                    }
                }
            }
            if let Err(e) = mem.store_all(&imps).await {
                error!(?e, "memory store failed");
            }
            prompt_builder.lock().await.add_impressions(&imps).await;
            let jitter = rand::thread_rng().gen_range(0..50);
            tokio::time::sleep(tick + Duration::from_millis(jitter)).await;
        }
    }
    /// Start the conversation and background tasks.
    ///
    /// All spawned tasks are tracked and aborted on drop. This ensures
    /// background loops do not outlive the [`Psyche`] if `run` is cancelled.
    /// Returns the updated [`Psyche`] when finished.
    pub async fn run(self) -> Self {
        info!("psyche run started");
        let buf = Arc::clone(&self.sensation_buffer);
        let observers = self.observers.clone();
        let bus = self.topic_bus.clone();
        let ticks = Arc::clone(&self.last_ticks);
        let memory = Arc::clone(&self.memory);
        let prompt_builder = Arc::clone(&self.prompt_builder);
        let pending = Arc::clone(&self.pending_turn);
        let tick = self.experience_tick;

        let mut tasks = TaskGroup::new();

        for wit in &self.wits {
            let wit = Arc::clone(wit);
            let ticks = Arc::clone(&ticks);
            let mem = Arc::clone(&memory);
            let prompt_builder = Arc::clone(&prompt_builder);
            let pending_turn = Arc::clone(&pending);
            let name = wit.name().to_string();

            let fut = AssertUnwindSafe(Self::wit_loop(
                wit,
                ticks,
                mem,
                prompt_builder,
                pending_turn,
                tick,
            ))
            .catch_unwind()
            .map(move |res| {
                if let Err(e) = res {
                    error!(%name, ?e, "wit loop panicked");
                }
            });
            tasks.spawn(fut);
        }

        tasks.spawn(Self::experience(
            buf,
            observers,
            bus,
            self.experience_tick,
            self.active_experience_tick,
            Arc::clone(&self.is_speaking),
        ));
        let converse_handle = tokio::spawn(self.converse());

        let psyche = match converse_handle.await {
            Ok(p) => p,
            Err(e) => {
                error!(?e, "converse task panicked");
                tasks.shutdown().await;
                panic!("converse task panicked: {e:?}");
            }
        };

        tasks.shutdown().await;
        info!("psyche run finished");
        psyche
    }
}
