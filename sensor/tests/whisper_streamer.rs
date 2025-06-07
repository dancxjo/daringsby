use sensor::whisper_streamer::{ASRResult, ASRStatus};

#[test]
fn result_serializes() {
    let res = ASRResult {
        transcript: "hi".into(),
        status: ASRStatus::Interim,
    };
    let json = serde_json::to_string(&res).unwrap();
    assert!(json.contains("interim"));
}
