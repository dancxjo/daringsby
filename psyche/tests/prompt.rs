use async_trait::async_trait;
use lingproc::{Chatter, Doer, LlmInstruction, Message, TextStream, Vectorizer};
use psyche::{
    Conversation, DEFAULT_SYSTEM_PROMPT, Ear, Impression, Mouth, PromptBuilder, Psyche, Stimulus,
    with_default_system_prompt,
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
}

#[test]
fn one_shot_prompt_includes_default_prompt() {
    let prompt = with_default_system_prompt("Summarize what I saw.");

    assert!(prompt.contains(DEFAULT_SYSTEM_PROMPT.trim()));
    assert!(prompt.contains("Task:\nSummarize what I saw."));
    assert!(prompt.contains("write in the first person"));
    assert!(prompt.contains("multiple frames from the same continuous stream"));
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
        }],
        summary: "greeting".into(),
        emoji: None,
        timestamp,
    };

    builder.add_impressions(&[impression.clone()]).await;
    let prompt = builder.build_prompt().await;

    assert!(prompt.contains(&impression.localized_timestamp()));
    assert!(prompt.contains("greeting"));
}
