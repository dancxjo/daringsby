use async_trait::async_trait;
use lingproc::Instruction;
use psyche::traits::Doer;
use psyche::{ImageData, Impression, Stimulus, VisionWit, Wit};
use std::sync::Arc;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _instruction: Instruction) -> anyhow::Result<String> {
        Ok("I see a test pattern.".into())
    }
}

#[tokio::test]
async fn captions_image() {
    let wit = Arc::new(VisionWit::new(Arc::new(Dummy)));
    wit.observe(ImageData {
        mime: "image/png".into(),
        base64: "zzz".into(),
    })
    .await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    let imp = &out[0];
    assert!(imp.summary.starts_with("I "));
    assert_eq!(imp.stimuli[0].what.mime, "image/png");
}
