pub mod ling;

use async_trait::async_trait;
use ling::{Chatter, Doer, Message, Vectorizer};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, broadcast, mpsc};
use tracing::{debug, error, info};

/// Default instructions sent to the language model.
///
/// The assistant should be concise, replying with no more than two
/// sentences. It will have additional opportunities to speak. Sending an
/// empty response indicates a pause where the assistant says nothing.
pub const DEFAULT_SYSTEM_PROMPT: &str = "Respond with one or two concise sentences at most. You will get another chance to speak. Returning an empty message means you remain silent.";

/// Event types emitted by the [`Psyche`] during conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A partial chunk of the assistant's response.
    StreamChunk(String),
    /// The assistant intends to say the given response.
    IntentionToSay(String),
    /// Base64-encoded WAV audio representing the spoken sentence.
    SpeechAudio(String),
}

/// Inputs that can be sent to a running [`Psyche`].
#[derive(Debug)]
pub enum Sensation {
    /// The assistant's speech was heard.
    HeardOwnVoice(String),
    /// The user spoke to the assistant.
    HeardUserVoice(String),
    /// Arbitrary input that the assistant can process
    Of(Box<dyn std::any::Any + Send + Sync>),
}

/// Something that can vocalize text.
#[async_trait]
pub trait Mouth: Send + Sync {
    /// Speak the provided text.
    async fn speak(&self, text: &str);
    /// Immediately stop saying anything queued or in progress.
    async fn interrupt(&self);
    /// Whether the mouth is currently speaking.
    fn speaking(&self) -> bool;
}

/// Something that can register what was said.
#[async_trait]
pub trait Ear: Send + Sync {
    /// The psyche heard itself say `text`.
    async fn hear_self_say(&self, text: &str);
    /// The psyche heard the user say `text`.
    async fn hear_user_say(&self, text: &str);
}

/// Simple conversation log.
#[derive(Default, Clone)]
pub struct Conversation {
    log: Vec<Message>,
}

impl Conversation {
    pub fn add_user(&mut self, content: String) {
        self.log.push(Message::user(content));
    }

    fn add_assistant(&mut self, content: String) {
        self.log.push(Message::assistant(content));
    }

    fn tail(&self, n: usize) -> Vec<Message> {
        let len = self.log.len();
        self.log[len.saturating_sub(n)..].to_vec()
    }

    /// Borrow the entire log.
    pub fn all(&self) -> &[Message] {
        &self.log
    }
}

/// The core AI engine.
pub struct Psyche {
    narrator: Box<dyn Doer>,
    voice: Box<dyn Chatter>,
    vectorizer: Box<dyn Vectorizer>,
    mouth: Arc<dyn Mouth>,
    ear: Arc<dyn Ear>,
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
}

impl Psyche {
    /// Construct a new [`Psyche`].
    pub fn new(
        narrator: Box<dyn Doer>,
        voice: Box<dyn Chatter>,
        vectorizer: Box<dyn Vectorizer>,
        mouth: Arc<dyn Mouth>,
        ear: Arc<dyn Ear>,
    ) -> Self {
        let (events_tx, _r) = broadcast::channel(16);
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        Self {
            narrator,
            voice,
            vectorizer,
            mouth,
            ear,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            max_history: 8,
            max_turns: 1,
            events_tx,
            input_tx,
            input_rx,
            conversation: Arc::new(Mutex::new(Conversation::default())),
            echo_timeout: Duration::from_secs(1),
            is_speaking: false,
            speak_when_spoken_to: false,
            pending_user_message: true,
        }
    }

    /// Change the system prompt.
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = prompt.into();
    }

    /// Access the current system prompt.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Limit the number of turns for a run.
    pub fn set_turn_limit(&mut self, turns: usize) {
        self.max_turns = turns;
    }

    /// Set how long to wait for an echo of what was said.
    pub fn set_echo_timeout(&mut self, dur: Duration) {
        self.echo_timeout = dur;
    }

    /// Subscribe to conversation events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events_tx.subscribe()
    }

    /// Sender for inputs to the running psyche.
    pub fn input_sender(&self) -> mpsc::UnboundedSender<Sensation> {
        self.input_tx.clone()
    }

    /// Broadcast channel for conversation events.
    pub fn event_sender(&self) -> broadcast::Sender<Event> {
        self.events_tx.clone()
    }

    /// Replace the current [`Mouth`] implementation.
    pub fn set_mouth(&mut self, mouth: Arc<dyn Mouth>) {
        self.mouth = mouth;
    }

    fn still_conversing(&self, turns: usize) -> bool {
        turns < self.max_turns
    }

    /// Access the conversation log.
    pub fn conversation(&self) -> Arc<Mutex<Conversation>> {
        self.conversation.clone()
    }

    /// Whether speech has been dispatched but not yet echoed.
    pub fn speaking(&self) -> bool {
        self.is_speaking
    }

    /// Require user input before speaking.
    pub fn set_speak_when_spoken_to(&mut self, enabled: bool) {
        self.speak_when_spoken_to = enabled;
        self.pending_user_message = !enabled;
    }

    /// Whether the psyche waits for the user before speaking.
    pub fn speak_when_spoken_to(&self) -> bool {
        self.speak_when_spoken_to
    }

    /// Main loop that handles the conversation with the assistant.
    async fn converse(mut self) -> Self {
        info!("psyche conversation started");
        let mut turns = 0;
        while self.still_conversing(turns) {
            if self.speak_when_spoken_to && !self.pending_user_message {
                match self.input_rx.recv().await {
                    Some(Sensation::HeardUserVoice(msg)) => {
                        debug!("heard user voice: {}", msg);
                        self.ear.hear_user_say(&msg).await;
                        let mut conv = self.conversation.lock().await;
                        conv.add_user(msg);
                        self.pending_user_message = true;
                        continue;
                    }
                    Some(Sensation::HeardOwnVoice(msg)) => {
                        debug!("heard own voice while waiting: {}", msg);
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
            if let Ok(mut stream) = self.voice.chat(&self.system_prompt, &history).await {
                use tokio_stream::StreamExt;
                let mut resp = String::new();
                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(chunk) => {
                            debug!("chunk received: {}", chunk);
                            let _ = self.events_tx.send(Event::StreamChunk(chunk.clone()));
                            resp.push_str(&chunk);
                        }
                        Err(_) => break,
                    }
                }
                info!("assistant intends to say: {}", resp);
                let _ = self.events_tx.send(Event::IntentionToSay(resp.clone()));
                self.is_speaking = true;
                self.mouth.speak(&resp).await;
                loop {
                    let recv = self.input_rx.recv();
                    match tokio::time::timeout(self.echo_timeout, recv).await {
                        Ok(Some(Sensation::HeardOwnVoice(msg))) => {
                            debug!("heard own voice: {}", msg);
                            self.ear.hear_self_say(&msg).await;
                            let mut conv = self.conversation.lock().await;
                            conv.add_assistant(msg);
                            self.is_speaking = false;
                            self.pending_user_message = !self.speak_when_spoken_to;
                            break;
                        }
                        Ok(Some(Sensation::HeardUserVoice(msg))) => {
                            debug!("heard user voice: {}", msg);
                            if self.is_speaking {
                                self.mouth.interrupt().await;
                                while self.input_rx.try_recv().is_ok() {}
                                self.is_speaking = false;
                            }
                            self.ear.hear_user_say(&msg).await;
                            let mut conv = self.conversation.lock().await;
                            conv.add_user(msg);
                            self.pending_user_message = true;
                            break;
                        }
                        Ok(Some(Sensation::Of(_))) => {
                            debug!("received non-voice sensation");
                            // TODO: handle other sensations
                        }
                        Ok(None) => {
                            self.pending_user_message = !self.speak_when_spoken_to;
                            break;
                        }
                        Err(_) => {
                            error!("echo timeout");
                            self.ear.hear_self_say(&resp).await;
                            let mut conv = self.conversation.lock().await;
                            conv.add_assistant(resp.clone());
                            self.is_speaking = false;
                            self.pending_user_message = !self.speak_when_spoken_to;
                            break;
                        }
                    }
                }
            } else {
                error!("voice chat failed");
                break;
            }
            turns += 1;
            debug!("turn {} complete", turns);
        }
        info!("psyche conversation ended");
        self
    }

    /// Background task processing non-conversational experience.
    async fn experience() {
        // Placeholder for future sensory processing.
        tokio::task::yield_now().await;
    }

    /// Run `converse` and `experience` concurrently and return the updated [`Psyche`].
    pub async fn run(self) -> Self {
        info!("psyche run started");
        let converse_handle = tokio::spawn(self.converse());
        let experience_handle = tokio::spawn(Self::experience());
        let psyche = converse_handle.await.expect("converse task panicked");
        experience_handle.await.expect("experience task panicked");
        info!("psyche run finished");
        psyche
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct Dummy;

    #[async_trait]
    impl Doer for Dummy {
        async fn follow(&self, _: &str) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Chatter for Dummy {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<ling::ChatStream> {
            Ok(Box::pin(tokio_stream::once(Ok("hi".to_string()))))
        }
    }

    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![1.0])
        }
    }
}
