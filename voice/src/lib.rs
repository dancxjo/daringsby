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
