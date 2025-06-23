use axum::{Router, routing::get, serve};
use futures::{SinkExt, StreamExt};
use pete::{Body, ChannelEar, EventBus, EyeSensor, GeoSensor, dummy_psyche, ws_handler};
use psyche::GeoLoc;
use psyche::traits::Sensor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};
use tokio::sync::mpsc;

#[tokio::test]
async fn websocket_forwards_geolocation() {
    let mut psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        Arc::new(AtomicBool::new(false)),
        psyche.voice(),
    ));
    // capture sensations sent by geo sensor
    let (tx, mut rx) = mpsc::channel(16);
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    let geo = Arc::new(GeoSensor::new(tx));
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
    let msg = serde_json::json!({
        "type": "Geolocate",
        "data": { "longitude": 1.0, "latitude": 2.0 }
    });
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            msg.to_string().into(),
        ))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let sensation = rx.try_recv().expect("no sensation received");
    if let psyche::Sensation::Of(any) = sensation {
        let loc = any.downcast_ref::<GeoLoc>().expect("wrong type");
        assert_eq!(loc.longitude, 1.0);
        assert_eq!(loc.latitude, 2.0);
    } else {
        panic!("unexpected sensation variant");
    }
    server.abort();
}
