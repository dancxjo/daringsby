//! Voice processing and language model interaction.
//!
//! The [`ChatVoice`] struct maintains a conversation history and streams
//! responses from an LLM.

use async_trait::async_trait;

#[derive(Debug, PartialEq, Eq)]
pub struct ThinkMessage {
    pub content: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SayMessage {
    pub content: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct EmoteMessage {
    pub emoji: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VoiceOutput {
    pub think: ThinkMessage,
    pub say: Option<SayMessage>,
    pub emote: Option<EmoteMessage>,
}

#[async_trait]
pub trait VoiceAgent: Send + Sync {
    /// Generate Pete's next thought based on the provided context.
    async fn narrate(&self, context: &str) -> VoiceOutput;
}

/// Print a debug message confirming the crate was loaded.
pub fn placeholder() {
    println!("voice module initialized");
}

pub mod context;
pub mod conversation;
pub mod model;
pub mod stream;
use conversation::{Conversation, Role};
use futures_util::StreamExt;
use model::ModelClient;
use regex::Regex;
use std::sync::Mutex;

/// Concrete [`VoiceAgent`] that streams chat completions from an LLM.
pub struct ChatVoice<C: ModelClient> {
    llm: C,
    conversation: Mutex<Conversation>,
    model: String,
}

impl<C: ModelClient> ChatVoice<C> {
    /// Create a new chat voice with a model name and conversation length.
    pub fn new(llm: C, model: impl Into<String>, max_history: usize) -> Self {
        Self {
            llm,
            conversation: Mutex::new(Conversation::new(max_history)),
            model: model.into(),
        }
    }

    /// Record a user message.
    pub fn receive_user(&self, msg: impl Into<String>) {
        let mut conv = self.conversation.lock().unwrap();
        conv.push(Role::User, msg);
    }
}

#[async_trait]
impl<C: ModelClient + Send + Sync> VoiceAgent for ChatVoice<C> {
    /// Generate a response from the LLM and update conversation history.
    async fn narrate(&self, context: &str) -> VoiceOutput {
        let prompt = {
            let conv = self.conversation.lock().unwrap();
            let mut prompt = format!("You are a storyteller narrating the life of Pete Daringsby. Narrate in the voice of Pete from the first person. Current thought: {context}\n");
            for m in conv.tail() {
                match m.role {
                    Role::Assistant => prompt.push_str(&format!("Pete: {}\n", m.content)),
                    Role::User => prompt.push_str(&format!("User: {}\n", m.content)),
                }
            }
            prompt
        };
        log::info!("Voice prompt: {}", prompt);
        let mut stream = match self.llm.stream_chat(&self.model, &prompt).await {
            Ok(s) => s,
            Err(_) => {
                return VoiceOutput {
                    think: ThinkMessage { content: String::new() },
                    say: None,
                    emote: None,
                }
            }
        };
        let mut response = String::new();
        while let Some(chunk) = stream.next().await {
            if let Ok(text) = chunk {
                response.push_str(&text);
            }
        }
        log::info!("Voice response: {}", response);
        let mut conv = self.conversation.lock().unwrap();
        conv.push(Role::Assistant, response.clone());
        let think_re = Regex::new(r"<think-silently>(.*?)</think-silently>").unwrap();
        let emote_re = Regex::new(r"<emote>(.*?)</emote>").unwrap();
        let mut think_content = String::new();
        for cap in think_re.captures_iter(&response) {
            if !think_content.is_empty() {
                think_content.push(' ');
            }
            think_content.push_str(&cap[1]);
        }
        let mut said = think_re.replace_all(&response, "").into_owned();
        let emote_msg = emote_re.captures(&said).map(|c| EmoteMessage { emoji: c[1].to_string() });
        said = emote_re.replace_all(&said, "").into_owned();
        let say_msg = if said.trim().is_empty() {
            None
        } else {
            Some(SayMessage { content: said.trim().to_string() })
        };
        VoiceOutput {
            think: ThinkMessage { content: think_content },
            say: say_msg,
            emote: emote_msg,
        }
    }
}
