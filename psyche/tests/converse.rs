use async_trait::async_trait;
use psyche::ling::{Chatter, Doer, Message, Vectorizer};
use psyche::{Event, Psyche, Sensation};

struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: &str) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<String> {
        Ok("hello world".into())
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[tokio::test]
async fn adds_message_after_voice_heard() {
    let mut psyche = Psyche::new(Box::new(Dummy), Box::new(Dummy), Box::new(Dummy));
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();
    let input = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    let mut saw_chunk = false;
    while let Ok(evt) = events.recv().await {
        match evt {
            Event::StreamChunk(_) => saw_chunk = true,
            Event::IntentionToSay(msg) => {
                input.send(Sensation::HeardOwnVoice(msg)).unwrap();
                break;
            }
        }
    }

    let psyche = handle.await.unwrap();
    assert!(saw_chunk);
    assert_eq!(psyche.conversation().all().len(), 1);
}
