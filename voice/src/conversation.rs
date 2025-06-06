use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Assistant,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

pub struct Conversation {
    messages: Vec<Message>,
    max_len: usize,
}

impl Conversation {
    pub fn new(max_len: usize) -> Self {
        Self { messages: Vec::new(), max_len }
    }

    pub fn push(&mut self, role: Role, content: impl Into<String>) {
        self.messages.push(Message { role, content: content.into() });
        if self.messages.len() > self.max_len {
            let excess = self.messages.len() - self.max_len;
            self.messages.drain(0..excess);
        }
    }

    pub fn tail(&self) -> &[Message] {
        &self.messages
    }
}
