//! Helper functions for building first-person prompts.
//!
//! These utilities keep the voice of Pete consistent across calls to the
//! language model.

/// Compose a simple prompt referencing current context and user input.
pub fn compose_prompt(here_and_now: &str, identity: &str, input: &str) -> String {
    format!("Pete Daringsby muses: {identity}\nCurrent thought: {here_and_now}\nUser said: {input}")
}
