/// Utilities to compose prompts for the language model in Pete's first-person voice.
pub fn compose_prompt(here_and_now: &str, identity: &str, input: &str) -> String {
    format!(
        "Pete Daringsby muses: {identity}\nCurrent thought: {here_and_now}\nUser said: {input}"
    )
}
