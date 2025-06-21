use psyche::{
    ImageData, Wit,
    wits::{DummyDetector, FaceWit, QdrantClient},
};
use std::sync::Arc;

#[tokio::test]
async fn detects_face() {
    let wit = Arc::new(FaceWit::new(
        Arc::new(DummyDetector),
        QdrantClient::default(),
    ));
    wit.observe(ImageData {
        mime: "image/png".into(),
        base64: "AA==".into(),
    })
    .await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].summary, "I'm seeing a face.");
    assert_eq!(out[0].stimuli.len(), 1);
}
