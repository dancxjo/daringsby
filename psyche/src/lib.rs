pub mod ling;

use async_trait::async_trait;
use ling::{Chatter, Doer, Message, Vectorizer};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};

/// Event types emitted by the [`Psyche`] during conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A partial chunk of the assistant's response.
    StreamChunk(String),
    /// The assistant intends to say the given response.
    IntentionToSay(String),
}

/// Inputs that can be sent to a running [`Psyche`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sensation {
    /// The assistant's speech was heard.
    HeardOwnVoice(String),
    /// The user spoke to the assistant.
    HeardUserVoice(String),
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
            system_prompt: String::new(),
            max_history: 8,
            max_turns: 1,
            events_tx,
            input_tx,
            input_rx,
            conversation: Arc::new(Mutex::new(Conversation::default())),
        }
    }

    /// Change the system prompt.
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = prompt.into();
    }

    /// Limit the number of turns for a run.
    pub fn set_turn_limit(&mut self, turns: usize) {
        self.max_turns = turns;
    }

    /// Subscribe to conversation events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events_tx.subscribe()
    }

    /// Sender for inputs to the running psyche.
    pub fn input_sender(&self) -> mpsc::UnboundedSender<Sensation> {
        self.input_tx.clone()
    }

    fn still_conversing(&self, turns: usize) -> bool {
        turns < self.max_turns
    }

    /// Access the conversation log.
    pub fn conversation(&self) -> Arc<Mutex<Conversation>> {
        self.conversation.clone()
    }

    /// Main loop that handles the conversation with the assistant.
    async fn converse(mut self) -> Self {
        let mut turns = 0;
        while self.still_conversing(turns) {
            let history = {
                let conv = self.conversation.lock().await;
                conv.tail(self.max_history)
            };
            if let Ok(resp) = self.voice.chat(&self.system_prompt, &history).await {
                for chunk in resp.split_whitespace() {
                    let _ = self.events_tx.send(Event::StreamChunk(chunk.to_string()));
                }
                let _ = self.events_tx.send(Event::IntentionToSay(resp.clone()));
                self.mouth.speak(&resp).await;
                loop {
                    match self.input_rx.recv().await {
                        Some(Sensation::HeardOwnVoice(msg)) => {
                            self.ear.hear_self_say(&msg).await;
                            let mut conv = self.conversation.lock().await;
                            conv.add_assistant(msg);
                            break;
                        }
                        Some(Sensation::HeardUserVoice(msg)) => {
                            if self.mouth.speaking() {
                                self.mouth.interrupt().await;
                            }
                            self.ear.hear_user_say(&msg).await;
                            let mut conv = self.conversation.lock().await;
                            conv.add_user(msg);
                        }
                        None => break,
                    }
                }
            } else {
                break;
            }
            turns += 1;
        }
        self
    }

    /// Background task processing non-conversational experience.
    async fn experience() {
        // Placeholder for future sensory processing.
        tokio::task::yield_now().await;
    }

    /// Run `converse` and `experience` concurrently and return the updated [`Psyche`].
    pub async fn run(self) -> Self {
        let converse_handle = tokio::spawn(self.converse());
        let experience_handle = tokio::spawn(Self::experience());
        let psyche = converse_handle.await.expect("converse task panicked");
        experience_handle.await.expect("experience task panicked");
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
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<String> {
            Ok("hi".into())
        }
    }

    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![1.0])
        }
    }
}
