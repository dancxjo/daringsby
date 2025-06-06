use async_trait::async_trait;

#[derive(Debug, PartialEq, Eq)]
pub struct ThinkMessage {
    pub content: String,
}

#[async_trait]
pub trait VoiceAgent: Send + Sync {
    async fn narrate(&self, context: &str) -> String;
}

pub fn placeholder() {
    println!("voice module initialized");
}

pub mod model;
pub mod context;
pub mod conversation;
use std::sync::Mutex;
use conversation::{Conversation, Role};
use model::ModelClient;
use futures_util::StreamExt;

pub struct ChatVoice<C: ModelClient> {
    llm: C,
    conversation: Mutex<Conversation>,
    model: String,
}

impl<C: ModelClient> ChatVoice<C> {
    pub fn new(llm: C, model: impl Into<String>, max_history: usize) -> Self {
        Self { llm, conversation: Mutex::new(Conversation::new(max_history)), model: model.into() }
    }

    pub fn receive_user(&self, msg: impl Into<String>) {
        let mut conv = self.conversation.lock().unwrap();
        conv.push(Role::User, msg);
    }
}

#[async_trait]
impl<C: ModelClient + Send + Sync> VoiceAgent for ChatVoice<C> {
    async fn narrate(&self, context: &str) -> String {
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
        let mut stream = match self.llm.stream_chat(&self.model, &prompt).await {
            Ok(s) => s,
            Err(_) => return String::new(),
        };
        let mut response = String::new();
        while let Some(chunk) = stream.next().await {
            if let Ok(text) = chunk {
                response.push_str(&text);
            }
        }
        let mut conv = self.conversation.lock().unwrap();
        conv.push(Role::Assistant, response.clone());
        response
    }
}
