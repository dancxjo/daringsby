use async_trait::async_trait;
use psyche::TrimMouth;
use psyche::traits::Mouth;
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
async fn trims_and_skips_empty() {
    let inner = Arc::new(RecordingMouth::default());
    let mouth = TrimMouth::new(inner.clone() as Arc<dyn Mouth>);

    mouth.speak("  hi  ").await;
    assert_eq!(inner.count.load(Ordering::SeqCst), 1);
    assert_eq!(inner.last.lock().unwrap().as_deref(), Some("hi"));

    mouth.speak("   ").await;
    assert_eq!(inner.count.load(Ordering::SeqCst), 1);
}
