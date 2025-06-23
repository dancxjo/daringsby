use async_trait::async_trait;
use cucumber::{World as _, given, then, when};
use pete::{ChannelEar, ChannelMouth, EventBus};
use psyche::{
    self, Ear, Event, Mouth,
    ling::{Chatter, Doer, Instruction, Message, TextStream, Vectorizer},
};
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::{Mutex, broadcast};
use tokio_stream::once;

#[derive(Default, cucumber::World)]
struct PipelineWorld {
    response: Option<String>,
    face: Option<String>,
    events: Option<broadcast::Receiver<Event>>,
    ear: Option<Arc<ChannelEar>>,
    convo: Option<Arc<Mutex<psyche::Conversation>>>,
    spoken: Vec<Event>,
}

impl std::fmt::Debug for PipelineWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineWorld").finish()
    }
}

#[derive(Clone)]
struct FixedLLM {
    reply: String,
}

#[async_trait]
impl Chatter for FixedLLM {
    async fn chat(&self, _s: &str, _h: &[Message]) -> anyhow::Result<TextStream> {
        Ok(Box::pin(once(Ok(self.reply.clone()))))
    }
    async fn update_prompt_context(&self, _c: &str) {}
}

#[async_trait]
impl Doer for FixedLLM {
    async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Vectorizer for FixedLLM {
    async fn vectorize(&self, _t: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

impl PipelineWorld {
    async fn start(&mut self) {
        if self.events.is_some() {
            return;
        }
        let (bus, _) = EventBus::new();
        let bus = Arc::new(bus);
        let speaking = Arc::new(AtomicBool::new(false));
        let mouth = Arc::new(ChannelMouth::new(bus.clone(), speaking.clone())) as Arc<dyn Mouth>;
        let llm = FixedLLM {
            reply: self.response.clone().unwrap_or_else(|| "hi".into()),
        };
        let mut psyche = psyche::Psyche::new(
            Box::new(llm.clone()),
            Box::new(llm.clone()),
            Box::new(llm),
            Arc::new(psyche::NoopMemory),
            mouth,
            Arc::new(psyche::NoopEar),
        );
        psyche.set_turn_limit(1);
        psyche.set_speak_when_spoken_to(true);
        let conversation = psyche.conversation();
        let voice = psyche.voice();
        let ear = Arc::new(ChannelEar::new(
            psyche.input_sender(),
            speaking,
            voice.clone(),
        ));
        let psyche = psyche.run();
        tokio::spawn(async move {
            psyche.await;
        });
        self.events = Some(bus.subscribe_events());
        self.ear = Some(ear);
        self.convo = Some(conversation);
    }

    async fn drain(&mut self) {
        if let Some(rx) = &mut self.events {
            while let Ok(e) = rx.try_recv() {
                self.spoken.push(e);
            }
        }
    }
}

#[given("Pete is running with an active interface")]
async fn given_running(w: &mut PipelineWorld) {
    w.start().await;
}

#[given(regex = "the front-end displays a default emoji (.+)")]
async fn given_face(w: &mut PipelineWorld, face: String) {
    w.face = Some(face);
}

#[given(regex = "the LLM is mocked to reply \"(.+)\" to .+")]
async fn given_llm(w: &mut PipelineWorld, reply: String) {
    w.response = Some(reply);
}

#[when(regex = "the user sends \"(.+)\"")]
async fn when_user(w: &mut PipelineWorld, msg: String) {
    w.start().await;
    if let Some(ear) = &w.ear {
        ear.hear_user_say(&msg).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    w.drain().await;
}

#[then(regex = "Pete says \"(.+)\"")]
async fn then_says(w: &mut PipelineWorld, msg: String) {
    assert!(w.spoken.iter().any(|e| match e {
        Event::Speech { text, .. } => text == &msg,
        _ => false,
    }));
}

#[then(regex = "the front-end emoji becomes (.+)")]
async fn then_emoji(w: &mut PipelineWorld, emo: String) {
    assert!(w.spoken.iter().any(|e| match e {
        Event::EmotionChanged(e) => e == &emo,
        _ => false,
    }));
}

#[then("no speech is produced")]
async fn then_no_speech(w: &mut PipelineWorld) {
    assert!(!w.spoken.iter().any(|e| matches!(e, Event::Speech { .. })));
}

#[tokio::main]
async fn main() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/features/pipeline.feature"
    );
    PipelineWorld::run(path).await;
}
