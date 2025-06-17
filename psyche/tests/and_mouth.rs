use async_trait::async_trait;
use psyche::{AndMouth, Mouth};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Default)]
struct CountingMouth {
    count: Arc<AtomicUsize>,
}

#[async_trait]
impl Mouth for CountingMouth {
    async fn speak(&self, _text: &str) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn broadcasts_to_all_mouths() {
    let a = Arc::new(CountingMouth::default());
    let b = Arc::new(CountingMouth::default());
    let mouth = AndMouth::new(vec![
        a.clone() as Arc<dyn Mouth>,
        b.clone() as Arc<dyn Mouth>,
    ]);
    mouth.speak("hi").await;
    assert_eq!(a.count.load(Ordering::SeqCst), 1);
    assert_eq!(b.count.load(Ordering::SeqCst), 1);
}
