use async_trait::async_trait;
use psyche::{Mouth, PlainMouth};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Default)]
struct RecordingMouth {
    count: Arc<AtomicUsize>,
    last: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl Mouth for RecordingMouth {
    async fn speak(&self, text: &str) {
        self.count.fetch_add(1, Ordering::SeqCst);
        *self.last.lock().unwrap() = Some(text.to_string());
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn strips_markdown() {
    let inner = Arc::new(RecordingMouth::default());
    let mouth = PlainMouth::new(inner.clone() as Arc<dyn Mouth>);

    mouth.speak("**hello** *world*!").await;
    assert_eq!(inner.count.load(Ordering::SeqCst), 1);
    assert_eq!(inner.last.lock().unwrap().as_deref(), Some("hello world!"));
}

#[tokio::test]
async fn skips_only_markup() {
    let inner = Arc::new(RecordingMouth::default());
    let mouth = PlainMouth::new(inner.clone() as Arc<dyn Mouth>);

    mouth.speak("**_**").await;
    assert_eq!(inner.count.load(Ordering::SeqCst), 0);
}
