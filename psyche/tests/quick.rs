use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::sensors::face::FaceInfo;
use psyche::traits::Doer;
use psyche::wits::Quick;
use psyche::{Heartbeat, ImageData, Sensation, Topic, TopicBus, Wit, image_content_id};
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
    bus.publish(Topic::Sensation, Sensation::heard_user_voice("hi"));
    sleep(Duration::from_millis(20)).await;
    let out = quick.tick().await;
    assert_eq!(out.len(), 1);
    assert!(out[0].summary.starts_with("SUM:"));
    assert!(out[0].summary.contains("in the first person"));
    assert!(out[0].summary.contains("using I/my/me"));
    assert!(out[0].summary.contains("Do not refer to Pete"));
    assert!(
        out[0]
            .summary
            .contains("consecutive frames from the same sensor stream")
    );
    assert_eq!(out[0].stimuli.len(), 1);
}

#[tokio::test]
async fn debug_report_uses_full_prompt() {
    let bus = TopicBus::new(8);
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    psyche::enable_debug("Quick").await;
    let quick = Quick::with_debug(bus, Arc::new(Dummy), Some(tx));
    let occurred_at = chrono::Utc::now() - chrono::Duration::seconds(1);
    quick
        .observe(Sensation::heard_user_voice_at("hi", occurred_at))
        .await;

    let out = quick.tick().await;
    let report = rx.recv().await.unwrap();

    assert_eq!(report.name, "Quick");
    assert!(report.prompt.contains(psyche::DEFAULT_SYSTEM_PROMPT.trim()));
    assert!(report.prompt.contains("using I/my/me"));
    assert!(
        report
            .prompt
            .contains(&out[0].stimuli[0].localized_timestamp())
    );
    assert!(report.prompt.contains("User said \"hi\""));
    psyche::disable_debug("Quick").await;
}

#[tokio::test]
async fn preserves_sensation_occurrence_time() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus, Arc::new(Dummy));
    let occurred_at = chrono::Utc::now() - chrono::Duration::seconds(1);
    quick
        .observe(Sensation::heard_user_voice_at("hi", occurred_at))
        .await;

    let out = quick.tick().await;

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].stimuli[0].timestamp, occurred_at);
}

#[tokio::test]
async fn describes_heartbeat_before_type_erasure() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus, Arc::new(Dummy));
    quick
        .observe(Sensation::of(Heartbeat {
            timestamp: chrono::Utc::now(),
        }))
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
    let crop = ImageData {
        mime: "image/png".into(),
        base64: "zzz".into(),
        captured_at: None,
    };
    quick
        .observe(Sensation::of(FaceInfo {
            face_id: image_content_id(&crop),
            source_image_id: image_content_id(&crop),
            crop,
            embedding: vec![0.1],
            vector_id: None,
        }))
        .await;

    let out = quick.tick().await;

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].stimuli[0].what, "I saw a face");
}

#[tokio::test]
async fn repeated_faces_are_framed_as_stream_frames() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus, Arc::new(Dummy));
    for _ in 0..2 {
        let crop = ImageData {
            mime: "image/png".into(),
            base64: "zzz".into(),
            captured_at: None,
        };
        quick
            .observe(Sensation::of(FaceInfo {
                face_id: image_content_id(&crop),
                source_image_id: image_content_id(&crop),
                crop,
                embedding: vec![0.1],
                vector_id: None,
            }))
            .await;
    }

    let out = quick.tick().await;

    assert_eq!(out.len(), 1);
    assert!(out[0].summary.contains("recent sensations"));
    assert!(
        out[0]
            .summary
            .contains("repeated similar camera or face observations")
    );
    assert_eq!(out[0].stimuli.len(), 2);
}

#[tokio::test]
async fn ignores_raw_image_frames() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus, Arc::new(Dummy));
    quick
        .observe(Sensation::of(ImageData {
            mime: "image/png".into(),
            base64: "zzz".into(),
            captured_at: None,
        }))
        .await;

    let out = quick.tick().await;

    assert!(out.is_empty());
}
