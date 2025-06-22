use psyche::Wit;
use psyche::sensors::face::FaceInfo;
use psyche::wits::face_memory_wit::FaceMemoryWit;
use std::sync::Arc;

fn dummy_info(val: f32) -> FaceInfo {
    FaceInfo {
        crop: psyche::ImageData {
            mime: "image/png".into(),
            base64: "".into(),
        },
        embedding: vec![val],
    }
}

#[tokio::test]
async fn recognizes_same_person() {
    let wit = Arc::new(FaceMemoryWit::new());
    wit.observe(dummy_info(0.1)).await;
    let out1 = wit.tick().await;
    assert_eq!(out1[0].summary, "I think someone new just showed up.");
    wit.observe(dummy_info(0.1)).await;
    let out2 = wit.tick().await;
    assert_eq!(out2[0].summary, "I saw the same person again.");
}

#[tokio::test]
async fn reports_no_faces() {
    let wit = Arc::new(FaceMemoryWit::new());
    for _ in 0..4 {
        let out = wit.tick().await;
        assert!(out.is_empty());
    }
    let out = wit.tick().await;
    assert_eq!(out[0].summary, "No faces detected for a while now.");
}
