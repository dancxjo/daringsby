use async_trait::async_trait;
use psyche::{Countenance, EmojiMouth, Mouth};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingMouth(Arc<Mutex<Option<String>>>);

#[async_trait]
impl Mouth for RecordingMouth {
    async fn speak(&self, text: &str) {
        *self.0.lock().unwrap() = Some(text.to_string());
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

#[derive(Clone, Default)]
struct RecordingFace(Arc<Mutex<Option<String>>>);

impl Countenance for RecordingFace {
    fn express(&self, emoji: &str) {
        *self.0.lock().unwrap() = Some(emoji.to_string());
    }
}

#[tokio::test]
async fn routes_emoji_to_face() {
    let mouth = Arc::new(RecordingMouth::default());
    let face = Arc::new(RecordingFace::default());
    let em = EmojiMouth::new(mouth.clone() as Arc<dyn Mouth>, face.clone());

    em.speak("hi 😊").await;

    assert_eq!(mouth.0.lock().unwrap().as_deref(), Some("hi"));
    assert_eq!(face.0.lock().unwrap().as_deref(), Some("😊"));
}

#[tokio::test]
async fn only_emoji_updates_face() {
    let mouth = Arc::new(RecordingMouth::default());
    let face = Arc::new(RecordingFace::default());
    let em = EmojiMouth::new(mouth.clone() as Arc<dyn Mouth>, face.clone());

    em.speak("😊😊").await;

    assert!(mouth.0.lock().unwrap().is_none());
    assert_eq!(face.0.lock().unwrap().as_deref(), Some("😊😊"));
}
