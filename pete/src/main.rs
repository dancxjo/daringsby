use psyche::{Psyche, PsycheEvent, PsycheInput};
use psyche::ling::{Chatter, InstructionFollower, Message, Vectorizer};
use async_trait::async_trait;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    #[derive(Clone)]
    struct Dummy;

    #[async_trait]
    impl InstructionFollower for Dummy {
        async fn follow(&self, _: &str) -> anyhow::Result<String> { Ok("ok".into()) }
    }

    #[async_trait]
    impl Chatter for Dummy {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<String> { Ok("hi".into()) }
    }

    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> { Ok(vec![0.0]) }
    }

    let narrator = Dummy;
    let voice = Dummy;
    let vectorizer = Dummy;

    let mut psyche = Psyche::new(Box::new(narrator), Box::new(voice), Box::new(vectorizer));
    let mut events = psyche.subscribe();
    let input = psyche.input_sender();
    let handle = tokio::spawn(async move { psyche.run().await });

    while let Ok(evt) = events.recv().await {
        match evt {
            PsycheEvent::StreamChunk(chunk) => print!("{chunk} "),
            PsycheEvent::IntentionToSay(msg) => {
                println!();
                input.send(PsycheInput::HeardOwnVoice(msg)).ok();
                break;
            }
        }
    }

    handle.await?;

    Ok(())
}
