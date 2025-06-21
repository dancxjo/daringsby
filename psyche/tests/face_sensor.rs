use psyche::{
    ImageData, Sensation, Sensor,
    wits::{
        face_sensor::{DummyDetector, FaceInfo, FaceSensor},
        memory::QdrantClient,
    },
};
use tokio::sync::mpsc;

#[tokio::test]
async fn emits_face_info() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sensor = FaceSensor::new(
        std::sync::Arc::new(DummyDetector::default()),
        QdrantClient::default(),
        tx,
    );
    sensor
        .sense(ImageData {
            mime: "image/png".into(),
            base64: "AA==".into(),
        })
        .await;
    let sensed = rx.recv().await.expect("no sensation");
    if let Sensation::Of(any) = sensed {
        let info = any
            .downcast_ref::<psyche::wits::face_sensor::FaceInfo>()
            .unwrap();
        assert_eq!(info.crop.mime, "image/png");
        assert_eq!(info.embedding, vec![0.0]);
    } else {
        panic!("wrong sensation")
    }
}
