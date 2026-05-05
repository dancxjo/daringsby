use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::sensors::face::FaceInfo;
use psyche::traits::Doer;
use psyche::wits::Quick;
use psyche::{Heartbeat, ImageData, Sensation, Topic, TopicBus, Wit};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, i: LlmInstruction) -> anyhow::Result<String> {
        Ok(format!("SUM:{}", i.command))
    }
}

#[tokio::test]
async fn summarizes_heard_text() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus.clone(), Arc::new(Dummy));
    // allow subscriber spawn
    sleep(Duration::from_millis(20)).await;
    bus.publish(Topic::Sensation, Sensation::HeardUserVoice("hi".into()));
    sleep(Duration::from_millis(20)).await;
    let out = quick.tick().await;
    assert_eq!(out.len(), 1);
    assert!(out[0].summary.starts_with("SUM:"));
    assert!(out[0].summary.contains("in the first person"));
    assert!(out[0].summary.contains("using I/my/me"));
    assert!(out[0].summary.contains("Do not refer to Pete"));
    assert_eq!(out[0].stimuli.len(), 1);
}

#[tokio::test]
async fn debug_report_uses_full_prompt() {
    let bus = TopicBus::new(8);
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    psyche::enable_debug("Quick").await;
    let quick = Quick::with_debug(bus, Arc::new(Dummy), Some(tx));
    quick.observe(Sensation::HeardUserVoice("hi".into())).await;

    let _ = quick.tick().await;
    let report = rx.recv().await.unwrap();

    assert_eq!(report.name, "Quick");
    assert!(report.prompt.contains(psyche::DEFAULT_SYSTEM_PROMPT.trim()));
    assert!(report.prompt.contains("using I/my/me"));
    psyche::disable_debug("Quick").await;
}

#[tokio::test]
async fn describes_heartbeat_before_type_erasure() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus, Arc::new(Dummy));
    quick
        .observe(Sensation::Of(Box::new(Heartbeat {
            timestamp: chrono::Utc::now(),
        })))
        .await;

    let out = quick.tick().await;

    assert_eq!(out.len(), 1);
    assert!(out[0].stimuli[0].what.starts_with("I felt a heartbeat at "));
    assert!(!out[0].summary.is_empty());
}

#[tokio::test]
async fn describes_faces_in_first_person() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus, Arc::new(Dummy));
    quick
        .observe(Sensation::Of(Box::new(FaceInfo {
            crop: ImageData {
                mime: "image/png".into(),
                base64: "zzz".into(),
            },
            embedding: vec![0.1],
        })))
        .await;

    let out = quick.tick().await;

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].stimuli[0].what, "I saw a face");
}

#[tokio::test]
async fn ignores_raw_image_frames() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus, Arc::new(Dummy));
    quick
        .observe(Sensation::Of(Box::new(ImageData {
            mime: "image/png".into(),
            base64: "zzz".into(),
        })))
        .await;

    let out = quick.tick().await;

    assert!(out.is_empty());
}
