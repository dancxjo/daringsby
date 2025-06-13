use crate::Experience;

/// Build a narrative summarizing recent experiences.
///
/// The prompt describes Pete's current situation and lists each
/// experience timestamped in chronological order.
pub fn narrative_prompt(context: &str, batch: &[Experience]) -> String {
    use chrono::{DateTime, Utc};
    let experiences = batch
        .iter()
        .map(|e| {
            let dt: DateTime<Utc> = e.when.into();
            format!("{} {}", dt.to_rfc3339(), e.how)
        })
        .collect::<Vec<_>>()
        .join(" ");
    let situation = if context.is_empty() {
        String::new()
    } else {
        format!(" Here's his present situation: {}", context)
    };
    format!(
        "You are acting as the linguistic processing unit for a larger entity named Pete. Assume the voice of Pete, an artificial consciousness; you are new to the world, so you should expect to sense new information slowly but surely. Just keep swimming.\n\nOver the past little while, you have experienced the following: {experiences}{situation}\n\nIn the voice of Pete and without headers or footers or any sort (just the plain text of Pete's response), produce a brief narrative from the perspective of Pete, talking to yourself, that compresses what's currently happening. Be succinct but thorough. Aim for one paragraph. Do not use bullet points or lists, just a single paragraph. Make sure to pass on the most important information from the experiences, but do not repeat them verbatim. Do not use any special formatting or markdown, just plain text."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrative_prompt_mentions_context() {
        let prompt = narrative_prompt("thinking", &[Experience::new("hi")]);
        assert!(prompt.contains("artificial consciousness"));
        assert!(prompt.contains("Here's his present situation: thinking"));
    }
}
