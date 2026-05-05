use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::traits::Doer;
use psyche::{ImageData, VisionWit, Wit};
use std::sync::Arc;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _instruction: LlmInstruction) -> anyhow::Result<String> {
        Ok("I see a test pattern.".into())
    }
}

#[tokio::test]
async fn captions_image() {
    let wit = Arc::new(VisionWit::new(Arc::new(Dummy)));
    wit.observe(ImageData {
        mime: "image/png".into(),
        base64: "zzz".into(),
        captured_at: None,
    })
    .await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    let imp = &out[0];
    assert!(imp.summary.starts_with("I "));
    assert_eq!(imp.stimuli[0].what.mime, "image/png");
}

#[tokio::test]
async fn tick_does_not_consume_latest_image() {
    let wit = Arc::new(VisionWit::new(Arc::new(Dummy)));
    wit.observe(ImageData {
        mime: "image/png".into(),
        base64: "zzz".into(),
        captured_at: None,
    })
    .await;

    assert_eq!(wit.tick().await.len(), 1);
    assert!(wit.latest_image_handle().lock().unwrap().is_some());
}

#[tokio::test]
async fn caption_stimulus_uses_image_capture_time() {
    let wit = Arc::new(VisionWit::new(Arc::new(Dummy)));
    let captured_at = "2026-05-05T12:34:56Z";
    wit.observe(ImageData {
        mime: "image/png".into(),
        base64: "zzz".into(),
        captured_at: Some(captured_at.into()),
    })
    .await;

    let out = wit.tick().await;

    assert_eq!(
        out[0].stimuli[0].timestamp.to_rfc3339(),
        "2026-05-05T12:34:56+00:00"
    );
}
