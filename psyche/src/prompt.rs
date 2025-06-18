//! Prompt building utilities for subagents.
//!
//! Each subagent constructs its LLM prompt via a dedicated struct
//! implementing [`PromptBuilder`]. These helpers centralize prompt
//! wording so it can be tweaked consistently.

/// Common interface for constructing prompts.
pub trait PromptBuilder {
    /// Build a prompt from `input`.
    fn build(&self, input: &str) -> String;
}

/// Prompt builder for the `Voice` subagent.
#[derive(Clone, Default)]
pub struct VoicePrompt;

impl PromptBuilder for VoicePrompt {
    fn build(&self, input: &str) -> String {
        input.to_string()
    }
}

/// Prompt builder for the `Will` subagent.
#[derive(Clone, Default)]
pub struct WillPrompt;

impl PromptBuilder for WillPrompt {
    fn build(&self, input: &str) -> String {
        format!("In one short sentence, what should Pete do or say next?\n{input}")
    }
}

/// Prompt builder for the `Heart` subagent.
#[derive(Clone, Default)]
pub struct HeartPrompt;

impl PromptBuilder for HeartPrompt {
    fn build(&self, input: &str) -> String {
        format!("Respond with a single emoji describing the overall emotion of:\n{input}")
    }
}
