use axum::{Router, routing::get, serve};
use futures::StreamExt;
use pete::{Body, ChannelEar, EventBus, EyeSensor, GeoSensor, dummy_psyche, ws_handler};
use psyche::Event;
use psyche::traits::Sensor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};

#[tokio::test]
async fn websocket_forwards_audio() {
    let mut psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let voice = psyche.voice();
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        Arc::new(AtomicBool::new(false)),
        voice,
    ));
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    psyche.add_sense(eye.description());
    psyche.add_sense(geo.description());
    let (bus, _user_rx) = EventBus::new();
    let bus = Arc::new(bus);
    let debug = psyche.debug_handle();
    let state = Body {
        bus: bus.clone(),
        ear,
        eye,
        geo,
        conversation,
        connections: Arc::new(AtomicUsize::new(0)),
        system_prompt: Arc::new(tokio::sync::Mutex::new(psyche.system_prompt())),
        psyche_debug: debug,
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        serve(listener, app.into_make_service()).await.unwrap();
    });

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", addr))
        .await
        .unwrap();
    bus.publish_event(Event::Speech {
        text: "hi".into(),
        audio: Some("UklGRg==".into()),
    });
    // skip initial system prompt
    let mut msg = socket.next().await.unwrap().unwrap();
    let mut value: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    if value["type"] == "SystemPrompt" {
        msg = socket.next().await.unwrap().unwrap();
        value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    }
    assert_eq!(value["type"], "Say");
    assert_eq!(value["data"]["audio"], "UklGRg==");
    assert_eq!(value["data"]["words"], "hi");
    server.abort();
}
