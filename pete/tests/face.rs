use pete::ChannelCountenance;
use psyche::{Countenance, Event};
use tokio::sync::broadcast;

#[tokio::test]
async fn broadcasts_emotion_changes() {
    let (tx, mut rx) = broadcast::channel(8);
    let face = ChannelCountenance::new(tx);
    face.express("😊");
    assert_eq!(rx.recv().await.unwrap(), Event::EmotionChanged("😊".into()));
}
