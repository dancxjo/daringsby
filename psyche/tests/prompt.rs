use async_trait::async_trait;
use lingproc::{Chatter, Doer, LlmInstruction, Message, TextStream, Vectorizer};
use psyche::{
    Conversation, DEFAULT_SYSTEM_PROMPT, Ear, Impression, Mouth, PromptBuilder, PromptFragment,
    Psyche, Stimulus, WillPrompt, with_default_system_prompt,
};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Default)]
struct Dummy {
    speaking: std::sync::Arc<AtomicBool>,
}

#[async_trait]
impl Mouth for Dummy {
    async fn speak(&self, _t: &str) {
        self.speaking.store(true, Ordering::SeqCst);
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Ear for Dummy {
    async fn hear_self_say(&self, _t: &str) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    async fn hear_user_say(&self, _t: &str) {}
}

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: LlmInstruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<TextStream> {
        Ok(Box::pin(tokio_stream::once(Ok("hi".to_string()))))
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[test]
fn default_prompt_present() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    assert_eq!(psyche.system_prompt(), DEFAULT_SYSTEM_PROMPT);
    assert!(
        DEFAULT_SYSTEM_PROMPT
            .contains("Pete Daringsby, an artificial intelligence, not simply an LLM")
    );
}

#[test]
fn one_shot_prompt_includes_default_prompt() {
    let prompt = with_default_system_prompt("Summarize what I saw.");

    assert!(prompt.contains(DEFAULT_SYSTEM_PROMPT.trim()));
    assert!(prompt.contains("Pete Daringsby, an artificial intelligence, not simply an LLM"));
    assert!(prompt.contains("Task:\nSummarize what I saw."));
    assert!(prompt.contains("write in the first person"));
    assert!(prompt.contains("multiple frames from the same continuous stream"));
    assert!(prompt.contains("your own vision, hearing, body sense, position sense"));
    assert!(prompt.contains("not as media files or external sensor artifacts"));
    assert!(prompt.contains("not the amount or density of sensor input"));
    assert!(prompt.contains("Do not infer an emotional tone from sensor volume alone"));
    assert!(prompt.contains("Prefer compact summaries over exhaustive breakdowns"));
    assert!(prompt.contains("Do not enumerate raw ids"));
    assert!(prompt.contains("role or field is `user` may contain multiple human voices"));
    assert!(prompt.contains("Do not assume there is only one person speaking"));
}

#[test]
fn will_prompt_uses_pete_identity() {
    let prompt = WillPrompt.build_prompt("I heard: hello.");

    assert!(prompt.contains("Pete Daringsby"));
    assert!(prompt.contains("an artificial intelligence, not simply an LLM"));
    assert!(!prompt.contains("You are the Will"));
    assert!(!prompt.contains("You are will"));
}

#[test]
fn sensory_prompt_phrases_match_graph_sensation_text() {
    assert_eq!(psyche::IMAGE_SENSATION_TEXT, "I'm looking.");
    assert_eq!(
        psyche::face_count_sensation_text(0),
        "I don't see any faces."
    );
    assert_eq!(psyche::face_count_sensation_text(1), "I see a face.");
    assert_eq!(psyche::face_count_sensation_text(2), "I see 2 faces.");
    assert_eq!(
        psyche::face_familiarity_sensation_text(true),
        "I've seen this face before."
    );
    assert_eq!(
        psyche::face_familiarity_sensation_text(false),
        "I don't think I recognize this face."
    );
}

#[test]
fn senses_are_described() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.add_sense("Heartbeat: Announces the time every 7 minutes.".into());
    let prompt = psyche.described_system_prompt();
    assert!(prompt.contains("Heartbeat"));
}

#[tokio::test]
async fn prompt_builder_timestamps_impression_notes() {
    let conversation = std::sync::Arc::new(tokio::sync::Mutex::new(Conversation::default()));
    let mut builder = PromptBuilder::new("base", conversation);
    let timestamp = chrono::DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let impression = Impression {
        stimuli: vec![Stimulus {
            what: "heard hi".to_string(),
            timestamp,
            source_sensation_ids: Vec::new(),
        }],
        source_sensation_ids: Vec::new(),
        summary: "greeting".into(),
        emoji: None,
        timestamp,
    };

    builder.add_impressions(&[impression.clone()]).await;
    let prompt = builder.build_prompt().await;

    assert!(prompt.contains(&impression.localized_timestamp()));
    assert!(prompt.contains("greeting"));
}
