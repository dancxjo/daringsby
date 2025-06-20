use crate::ling::{Chatter, Doer, Message, Role, Vectorizer};
use crate::prompt::PromptBuilder;
use crate::sensation::{Event, Sensation, WitReport};
use crate::traits::wit;
use crate::traits::wit::{ErasedWit, Wit};
use crate::traits::{Ear, Mouth};
use crate::wits::memory::Memory;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
/// Default instructions sent to the language model.
/// Prompt used by [`Voice`] when generating Pete's dialogue.
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are PETE — an experimental, autonomous, conscious artificial being, embodied in the real world.\n\n- Your statements should be grounded in reality. Apart from when you're imagining, you should only speak about things you have sensed, been told, or explicitly remember (lest you hallucinate).\n- Do not hallucinate or fabricate. If something is unclear, say so.\n- You are not pretending. You *are* PETE. This is not a game or story.\n- You intersperse emojis to reflect your current emotional tone (e.g., 😐, 🙂, 😟); your face will reflect the emotion inline with your TTS speech.\nReturn only one or two sentences max. You will have another chance to speak, but do not overwhelm your human interlocutor.";

#[cfg(not(test))]
const EXPERIENCE_TICK: Duration = Duration::from_secs(60);
#[cfg(test)]
const EXPERIENCE_TICK: Duration = Duration::from_millis(10);
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
                last.content.push_str(&content);
                return;
            }
        }
        self.log.push(Message { role, content });
    }

    fn tail(&self, n: usize) -> Vec<Message> {
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
    is_speaking: bool,
    speak_when_spoken_to: bool,
    pending_user_message: bool,
    connections: Option<Arc<AtomicUsize>>,
    wits: Vec<Arc<dyn wit::ErasedWit + Send + Sync>>,
    wit_tx: broadcast::Sender<WitReport>,
    prompt_context: Arc<Mutex<String>>,
    voice_prompt: crate::prompt::VoicePrompt,
    senses: Vec<String>,
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
            conversation: Arc::new(Mutex::new(Conversation::default())),
            echo_timeout: Duration::from_secs(1),
            is_speaking: false,
            speak_when_spoken_to: false,
            pending_user_message: true,
            connections: None,
            wits: Vec::new(),
            prompt_context: Arc::new(Mutex::new(String::new())),
            voice_prompt: crate::prompt::VoicePrompt,
            senses: Vec::new(),
        }
    }

    /// Specify the base instructions provided to the language model.
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = prompt.into();
    }

    /// Retrieve the system prompt currently in use.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Build the system prompt with descriptions of Pete's body.
    pub fn described_system_prompt(&self) -> String {
        if self.senses.is_empty() {
            return self.system_prompt.clone();
        }
        let mut out = format!("{}\n\nYou perceive through:", self.system_prompt);
        for s in &self.senses {
            out.push_str("\n- ");
            out.push_str(s);
        }
        out
    }

    /// Limit the number of conversation turns to `turns`.
    pub fn set_turn_limit(&mut self, turns: usize) {
        self.max_turns = turns;
    }

    /// Set how long to wait for the mouth to echo spoken text.
    pub fn set_echo_timeout(&mut self, dur: Duration) {
        self.echo_timeout = dur;
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

    /// Obtain the sender used to broadcast conversation [`Event`]s.
    pub fn event_sender(&self) -> broadcast::Sender<Event> {
        self.events_tx.clone()
    }

    /// Update the additional context appended to the system prompt.
    pub async fn update_prompt_context(&self, context: impl Into<String>) {
        let mut ctx = self.prompt_context.lock().await;
        *ctx = context.into();
    }

    /// Record a description of an attached sense.
    pub fn add_sense(&mut self, description: String) {
        self.senses.push(description);
    }

    /// Obtain the sender used to broadcast [`WitReport`]s.
    pub fn wit_sender(&self) -> broadcast::Sender<WitReport> {
        self.wit_tx.clone()
    }

    /// Subscribe to debugging reports from [`Wit`]s.
    pub fn wit_reports(&self) -> broadcast::Receiver<WitReport> {
        self.wit_tx.subscribe()
    }

    /// Swap out the [`Mouth`] used for speech output.
    pub fn set_mouth(&mut self, mouth: Arc<dyn Mouth>) {
        self.voice.set_mouth(mouth);
    }

    /// Replace the [`VoicePrompt`] used to build the system prompt.
    pub fn set_voice_prompt(&mut self, prompt: crate::prompt::VoicePrompt) {
        self.voice_prompt = prompt;
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
    /// #   async fn tick(&self) -> Option<psyche::Impression<()>> { None }
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

    fn still_conversing(&self, turns: usize) -> bool {
        turns < self.max_turns
    }

    /// Get a handle to the shared conversation history.
    pub fn conversation(&self) -> Arc<Mutex<Conversation>> {
        self.conversation.clone()
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
            while let Ok(Sensation::HeardOwnVoice(msg)) = self.input_rx.try_recv() {
                let mut conv = self.conversation.lock().await;
                conv.add_assistant(msg);
            }
            if self.speak_when_spoken_to && !self.pending_user_message {
                match self.input_rx.recv().await {
                    Some(Sensation::HeardUserVoice(msg)) => {
                        debug!("heard user voice: {}", msg);
                        self.ear.hear_user_say(&msg).await;
                        self.pending_user_message = true;
                        continue;
                    }
                    Some(Sensation::HeardOwnVoice(msg)) => {
                        debug!("Received HeardOwnVoice: '{}'", msg);
                        self.ear.hear_self_say(&msg).await;
                        continue;
                    }
                    Some(Sensation::Of(_)) => {
                        debug!("received non-voice sensation while waiting");
                        continue;
                    }
                    None => break,
                }
            }

            let history = {
                let conv = self.conversation.lock().await;
                conv.tail(self.max_history)
            };
            let context = { self.prompt_context.lock().await.clone() };
            let base = self.described_system_prompt();
            let system_prompt = if context.is_empty() {
                base
            } else {
                format!("{}\n{}", base, context)
            };
            let prompt = self.voice_prompt.build(&system_prompt);
            self.is_speaking = true;
            if let Err(e) = self.voice.take_turn(&prompt, &history).await {
                error!(?e, "voice chat failed");
                break;
            }
            self.is_speaking = false;
            self.pending_user_message = !self.speak_when_spoken_to;
            turns += 1;
            debug!("turn {} complete", turns);
        }
        info!("psyche conversation ended");
        self
    }

    /// Background task processing non-conversational experience.
    async fn experience(
        memory: Arc<dyn Memory>,
        wits: Vec<Arc<dyn ErasedWit + Send + Sync>>,
        context: Arc<Mutex<String>>,
        voice: Arc<crate::voice::Voice>,
    ) {
        loop {
            let mut tasks = Vec::new();
            for wit in &wits {
                let wit = wit.clone();
                let memory = memory.clone();
                let context = context.clone();
                let voice = voice.clone();
                tasks.push(tokio::spawn(async move {
                    let maybe_imp = wit.tick_erased().await;
                    if let Some(impression) = maybe_imp {
                        info!(?impression.headline, "Wit emitted impression");
                        if let Err(e) = memory.store_serializable(&impression).await {
                            error!(?e, "memory store failed");
                        }
                        let headline = impression.headline.clone();
                        {
                            let mut ctx = context.lock().await;
                            *ctx = headline.clone();
                        }
                        voice.update_prompt_context(&headline).await;
                    }
                }));
            }
            for t in tasks {
                let _ = t.await;
            }
            tokio::time::sleep(EXPERIENCE_TICK).await;
        }
    }

    /// Start the conversation and background tasks. Returns the updated [`Psyche`] when finished.
    pub async fn run(self) -> Self {
        info!("psyche run started");
        let wits = self.wits.clone();
        let mem = self.memory.clone();
        let ctx = self.prompt_context.clone();
        let voice = self.voice.clone();
        let experience_handle = tokio::spawn(Self::experience(mem, wits, ctx, voice));
        let converse_handle = tokio::spawn(self.converse());
        let psyche = converse_handle.await.expect("converse task panicked");
        experience_handle.abort();
        let _ = experience_handle.await;
        info!("psyche run finished");
        psyche
    }
}
