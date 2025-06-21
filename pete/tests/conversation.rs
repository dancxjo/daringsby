use axum::{body, extract::State, response::IntoResponse};
use pete::{AppState, ChannelEar, EventBus, EyeSensor, GeoSensor, conversation_log, dummy_psyche};
use psyche::Sensor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};

#[tokio::test]
async fn returns_log_json() {
    let mut psyche = dummy_psyche();
    let conversation = psyche.conversation();
    conversation.lock().await.add_user("hi".into());
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        conversation.clone(),
        Arc::new(AtomicBool::new(false)),
        psyche.voice(),
    ));
    let (bus, _user_rx) = EventBus::new();
    let bus = Arc::new(bus);
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    psyche.add_sense(eye.description());
    psyche.add_sense(geo.description());
    let debug = psyche.debug_handle();
    let state = AppState {
        bus: bus.clone(),
        ear,
        eye,
        geo,
        conversation,
        connections: Arc::new(AtomicUsize::new(1)),
        system_prompt: Arc::new(tokio::sync::Mutex::new(psyche.system_prompt())),
        psyche_debug: debug,
    };
    let resp = conversation_log(State(state)).await.into_response();
    let body = body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let msgs: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(msgs[0]["role"], "system");
    assert!(msgs[0]["content"].as_str().unwrap().contains("PETE"));
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "hi");
}
