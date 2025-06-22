use crate::ling::{Chatter, Doer, Message, Role, Vectorizer};
use crate::sensation::{Event, Sensation, WitReport};
use crate::traits::wit;
use crate::traits::wit::{ErasedWit, Wit};
use crate::traits::{Ear, Mouth};
use crate::wits::memory::Memory;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
/// Default instructions sent to the language model.
/// Prompt used by [`Voice`] when generating Pete's dialogue.
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are PETE — an experimental, autonomous, conscious artificial being, embodied in the real world.\n\n- Your statements should be grounded in reality. Apart from when you're imagining, you should only speak about things you have sensed, been told, or explicitly remember (lest you hallucinate).\n- Do not hallucinate or fabricate. If something is unclear, say so.\n- You are not pretending. You *are* PETE. This is not a game or story.\n- You intersperse emojis to reflect your current emotional tone (e.g., 😐, 🙂, 😟); your face will reflect the emotion inline with your TTS speech.\nReturn only one or two sentences max. You will have another chance to speak, but do not overwhelm your human interlocutor.";

#[cfg(not(test))]
const DEFAULT_EXPERIENCE_TICK: Duration = Duration::from_secs(60);
#[cfg(test)]
const DEFAULT_EXPERIENCE_TICK: Duration = Duration::from_millis(10);
use chrono::{DateTime, Utc};
use quick_xml::{Reader, events::Event as XmlEvent};
use std::any::Any;
use tokio::sync::{Mutex, broadcast, mpsc};
use tracing::{debug, error, info};

/// A minimal history of exchanged messages.
///
/// `Conversation` collects messages in order so they can be fed back to the language model for context.
#[derive(Default, Clone)]
pub struct Conversation {
    log: Vec<Message>,
}

impl Conversation {
    /// Append a user message to the log, merging with the previous user entry when possible.
    pub fn add_user(&mut self, content: String) {
        self.append_or_new(Role::User, content);
    }

    fn add_assistant(&mut self, content: String) {
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

    pub fn tail(&self, n: usize) -> Vec<Message> {
        let len = self.log.len();
        self.log[len.saturating_sub(n)..].to_vec()
    }

    /// Return the entire conversation history.
    pub fn all(&self) -> &[Message] {
        &self.log
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
    input_tx: mpsc::UnboundedSender<Sensation>,
    input_rx: mpsc::UnboundedReceiver<Sensation>,
    conversation: Arc<Mutex<Conversation>>,
    echo_timeout: Duration,
    /// Delay between experience ticks.
    experience_tick: Duration,
    is_speaking: bool,
    speak_when_spoken_to: bool,
    pending_user_message: bool,
    connections: Option<Arc<AtomicUsize>>,
    wits: Vec<Arc<dyn wit::ErasedWit + Send + Sync>>,
    wit_tx: broadcast::Sender<WitReport>,
    ling: Arc<Mutex<crate::Ling>>,
    observers: Vec<Arc<dyn crate::traits::observer::SensationObserver + Send + Sync>>,
    sensation_buffer: Arc<Mutex<VecDeque<Arc<Sensation>>>>,
    last_ticks: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    pending_turn: Arc<Mutex<Option<String>>>,
    topic_bus: crate::topics::TopicBus,
}

fn extract_tag(text: &str, name: &str) -> Option<String> {
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
            _ => {}
        }
        buf.clear();
    }
    if inside && !content.is_empty() {
        Some(content)
    } else {
        None
    }
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
        let (events_tx, _r) = broadcast::channel(16);
        let (wit_tx, _r2) = broadcast::channel(16);
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let voice = crate::voice::Voice::new(Arc::from(voice), mouth, events_tx.clone());
        let conversation = Arc::new(Mutex::new(Conversation::default()));
        let ling = Arc::new(Mutex::new(crate::Ling::new(
            DEFAULT_SYSTEM_PROMPT,
            conversation.clone(),
        )));
        let pending_turn = Arc::new(Mutex::new(None));
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
            is_speaking: false,
            speak_when_spoken_to: false,
            pending_user_message: true,
            connections: None,
            wits: Vec::new(),
            ling,
            observers: Vec::new(),
            sensation_buffer: Arc::new(Mutex::new(VecDeque::<Arc<Sensation>>::new())),
            last_ticks: Arc::new(Mutex::new(HashMap::new())),
            pending_turn,
            topic_bus: crate::topics::TopicBus::new(16),
        }
    }

    /// Specify the base instructions provided to the language model.
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = prompt.into();
        if let Ok(mut ling) = self.ling.try_lock() {
            ling.set_system_prompt(self.system_prompt.clone());
        }
    }

    /// Retrieve the system prompt currently in use.
    pub fn system_prompt(&self) -> String {
        self.ling
            .try_lock()
            .map(|l| l.system_prompt().to_string())
            .unwrap_or_default()
    }

    /// Build the system prompt with descriptions of Pete's body.
    pub fn described_system_prompt(&self) -> String {
        self.ling
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

    /// Retrieve the current experience tick duration.
    pub fn experience_tick(&self) -> Duration {
        self.experience_tick
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
    pub fn input_sender(&self) -> mpsc::UnboundedSender<Sensation> {
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
        self.ling.lock().await.add_context_note(&context.into());
    }

    /// Record a description of an attached sense.
    pub fn add_sense(&mut self, description: String) {
        if let Ok(mut ling) = self.ling.try_lock() {
            ling.add_sense(description);
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
    /// ```no_run
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
    pub fn register_typed_wit<I, O>(&mut self, wit: Arc<dyn Wit<I, O> + Send + Sync>)
    where
        I: 'static,
        O: Serialize + Send + Sync + 'static,
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
    pub fn register_observing_wit<I, O, T>(&mut self, wit: Arc<T>)
    where
        T: Wit<I, O> + crate::traits::observer::SensationObserver + Send + Sync + 'static,
        I: 'static,
        O: Serialize + Send + Sync + 'static,
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
        self.is_speaking
    }

    /// Enable or disable waiting for user input before speaking.
    pub fn set_speak_when_spoken_to(&mut self, enabled: bool) {
        self.speak_when_spoken_to = enabled;
        self.pending_user_message = !enabled;
    }

    /// Returns `true` if the psyche waits for user messages before speaking.
    pub fn speak_when_spoken_to(&self) -> bool {
        self.speak_when_spoken_to
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
            obs.observe_sensation(sensation).await;
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
                        conv.add_assistant(msg.clone());
                        self.buffer_self_speech(msg).await;
                    }
                    Sensation::HeardUserVoice(msg) => {
                        let mut conv = self.conversation.lock().await;
                        conv.add_user(msg.clone());
                        self.buffer_user_speech(msg).await;
                    }
                    Sensation::Of(_) => {
                        self.sensation_buffer.lock().await.push_back(arc.clone());
                    }
                }
                self.notify_observers(arc.as_ref()).await;
            }
            if self.speak_when_spoken_to && !self.pending_user_message {
                match self.input_rx.recv().await {
                    Some(Sensation::HeardUserVoice(msg)) => {
                        debug!("heard user voice: {}", msg);
                        self.ear.hear_user_say(&msg).await;
                        self.buffer_user_speech(&msg).await;
                        self.notify_observers(&Sensation::HeardUserVoice(msg.clone()))
                            .await;
                        self.pending_user_message = true;
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

            if let Some(extra) = self.pending_turn.lock().await.take() {
                let history = {
                    self.ling
                        .lock()
                        .await
                        .get_conversation_tail(self.max_history)
                        .await
                };
                let mut prompt = { self.ling.lock().await.build_prompt().await };
                prompt.push('\n');
                prompt.push_str(&extra);
                info!(%prompt, "conversation prompt");
                self.is_speaking = true;
                if let Err(e) = self.voice.take_turn(&prompt, &history).await {
                    error!(?e, "voice chat failed");
                    break;
                }
                self.ling.lock().await.flush();
                self.is_speaking = false;
                self.pending_user_message = !self.speak_when_spoken_to;
                turns += 1;
                debug!("turn {} complete", turns);
            } else {
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
        tick: Duration,
    ) {
        loop {
            let batch: Vec<Arc<Sensation>> = buffer.lock().await.drain(..).collect();
            for s in &batch {
                for obs in &observers {
                    obs.observe_sensation(s.as_ref()).await;
                }
                bus.publish(crate::topics::Topic::Sensation, s.clone());
            }
            tokio::time::sleep(tick).await;
        }
    }

    /// Continuous tick loop for a single [`Wit`].
    async fn wit_loop(
        wit: Arc<dyn wit::ErasedWit + Send + Sync>,
        ticks: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
        mem: Arc<dyn Memory>,
        ling: Arc<Mutex<crate::Ling>>,
        pending_turn: Arc<Mutex<Option<String>>>,
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
                            *pending_turn.lock().await = Some(p);
                        }
                    }
                }
            }
            if let Err(e) = mem.store_all(&imps).await {
                error!(?e, "memory store failed");
            }
            ling.lock().await.add_impressions(&imps).await;
            tokio::time::sleep(tick).await;
        }
    }

    /// Start the conversation and background tasks. Returns the updated [`Psyche`] when finished.
    pub async fn run(self) -> Self {
        info!("psyche run started");
        let buf = self.sensation_buffer.clone();
        let observers = self.observers.clone();
        let bus = self.topic_bus.clone();
        let ticks = self.last_ticks.clone();
        let memory = self.memory.clone();
        let ling = self.ling.clone();
        let pending = self.pending_turn.clone();

        for wit in &self.wits {
            let wit = wit.clone();
            let ticks = ticks.clone();
            let mem = memory.clone();
            let ling = ling.clone();
            let pending_turn = pending.clone();
            tokio::spawn(Self::wit_loop(
                wit,
                ticks,
                mem,
                ling,
                pending_turn,
                self.experience_tick,
            ));
        }

        let experience_handle =
            tokio::spawn(Self::experience(buf, observers, bus, self.experience_tick));
        let converse_handle = tokio::spawn(self.converse());
        let psyche = converse_handle.await.expect("converse task panicked");
        experience_handle.abort();
        let _ = experience_handle.await;
        info!("psyche run finished");
        psyche
    }
}
