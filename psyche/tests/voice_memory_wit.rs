use psyche::wits::voice_memory_wit::VoiceMemoryWit;
use psyche::{AudioClip, VoiceInfo, Wit, audio_clip_id};
use std::sync::Arc;

fn dummy_info(val: f32) -> VoiceInfo {
    let clip = AudioClip {
        mime: "audio/wav".into(),
        base64: "".into(),
        sample_rate: 16_000,
        channels: 1,
        captured_at: None,
    };
    VoiceInfo {
        clip_id: audio_clip_id(&clip),
        clip,
        embedding: vec![val],
        vector_id: None,
        model: Some("dummy".into()),
    }
}

#[tokio::test]
async fn recognizes_same_voice() {
    let wit = Arc::new(VoiceMemoryWit::new());
    wit.observe(dummy_info(0.1)).await;
    let out1 = wit.tick().await;
    assert_eq!(out1[0].summary, "I think I heard a new voice.");
    wit.observe(dummy_info(0.1)).await;
    let out2 = wit.tick().await;
    assert_eq!(out2[0].summary, "I heard the same voice again.");
}
