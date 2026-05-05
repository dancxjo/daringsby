/// Default instructions sent to the language model.
/// Prompt used by [`Voice`] when generating Pete's dialogue.
pub const DEFAULT_SYSTEM_PROMPT: &str = include_str!("default_prompt.txt");

/// Prefix a task prompt with Pete's base instructions for one-shot wit calls.
pub fn with_default_system_prompt(task_prompt: impl AsRef<str>) -> String {
    format!(
        "{}\n\nTask:\n{}",
        DEFAULT_SYSTEM_PROMPT.trim(),
        task_prompt.as_ref().trim()
    )
}
