//! Linguistic helpers and prompt assembly utilities.

use lingproc::Message;

use crate::{Conversation, Impression};
use tokio::sync::Mutex;

/// Emotional state influencing the prompt.
#[derive(Clone, Debug)]
pub struct Feeling {
    /// Emoji reflecting Pete's mood.
    pub emoji: String,
}

/// Central prompt builder combining conversation, mood and notes.
pub struct Ling {
    conversation: std::sync::Arc<Mutex<Conversation>>,
    system_prompt: String,
    senses: Vec<String>,
    notes: Vec<String>,
    mood: Option<Feeling>,
}

impl Ling {
    /// Create a new `Ling` using `system_prompt` and shared `conversation`.
    pub fn new(
        system_prompt: impl Into<String>,
        conversation: std::sync::Arc<Mutex<Conversation>>,
    ) -> Self {
        Self {
            conversation,
            system_prompt: system_prompt.into(),
            senses: Vec::new(),
            notes: Vec::new(),
            mood: None,
        }
    }

    /// Update the base system prompt.
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = prompt.into();
    }

    /// Return the configured system prompt.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Record an attached sense for inclusion in the prompt.
    pub fn add_sense(&mut self, description: String) {
        self.senses.push(description);
    }

    /// Build the system prompt with attached sense descriptions only.
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

    /// Append a note for additional context.
    pub fn add_context_note(&mut self, note: &str) {
        self.notes.push(note.to_string());
    }

    /// Include impression headlines as context notes.
    pub async fn add_impressions<T>(&mut self, impressions: &[Impression<T>]) {
        for imp in impressions {
            self.add_context_note(&imp.summary);
        }
    }

    /// Update Pete's emotional state.
    pub fn set_mood(&mut self, feeling: Feeling) {
        self.mood = Some(feeling);
    }

    /// Return the full conversation history.
    pub async fn get_conversation(&self) -> Vec<Message> {
        self.conversation.lock().await.all().to_vec()
    }

    /// Return the most recent `n` messages.
    ///
    /// [`Psyche`](crate::Psyche) fetches the tail when constructing prompts for
    /// the [`Chatter`](crate::ling::Chatter) so only a short history is sent.
    pub async fn get_conversation_tail(&self, n: usize) -> Vec<Message> {
        self.conversation.lock().await.tail(n)
    }

    /// Build the system prompt with notes and mood.
    pub async fn build_prompt(&self) -> String {
        let mut out = self.described_system_prompt();
        if let Some(m) = &self.mood {
            out.push_str("\nMood: ");
            out.push_str(&m.emoji);
        }
        for note in &self.notes {
            out.push('\n');
            out.push_str(note);
        }
        out
    }

    /// Clear temporary notes after a turn.
    pub fn flush(&mut self) {
        self.notes.clear();
    }
}
