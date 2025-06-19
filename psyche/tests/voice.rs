use async_trait::async_trait;
use psyche::ling::{ChatStream, Chatter, Doer, Instruction, Message};
use psyche::{Event, Mouth};
use psyche::{Voice, extract_emojis};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::once;

#[derive(Clone, Default)]
struct DummyLLM;

#[async_trait]
impl Chatter for DummyLLM {
    async fn chat(&self, _s: &str, _h: &[Message]) -> anyhow::Result<ChatStream> {
        Ok(Box::pin(once(Ok("Hi 😊".to_string()))))
    }
    async fn update_prompt_context(&self, _c: &str) {}
}

#[async_trait]
impl Doer for DummyLLM {
    async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[derive(Clone, Default)]
struct RecMouth(Arc<tokio::sync::Mutex<Vec<String>>>);

#[async_trait]
impl Mouth for RecMouth {
    async fn speak(&self, t: &str) {
        self.0.lock().await.push(t.to_string());
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

#[test]
fn extract_emojis_splits() {
    let (text, e) = extract_emojis("hello 😊");
    assert_eq!(text, "hello");
    assert_eq!(e, vec!["😊"]);
}

// TODO: Fix broken tests
// #[tokio::test]
// async fn take_turn_routes_emojis() {
//     let mouth = Arc::new(RecMouth::default());
//     let (tx, mut rx) = broadcast::channel(8);
//     let voice = Voice::new(Arc::new(DummyLLM), mouth.clone(), tx);
//     voice.permit(None);
//     voice.take_turn("sys", &[]).await.unwrap();
//     assert_eq!(mouth.0.lock().await.as_slice(), ["Hi"]);
//     // first event should be emotion changed
//     let mut saw_emotion = false;
//     while let Ok(evt) = rx.try_recv() {
//         if let Event::EmotionChanged(e) = evt {
//             saw_emotion = e == "😊";
//             break;
//         }
//     }
//     assert!(saw_emotion);
// }
