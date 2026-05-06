use async_trait::async_trait;
use axum::{Router, routing::get, serve};
use futures::{SinkExt, StreamExt};
use pete::{Body, EventBus, EyeSensor, GeoSensor, MotionSensor, dummy_psyche, ws_handler};
use psyche::{BrowserMotion, ImageData, Sensor, traits::Ear};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{Mutex, mpsc};

struct RecordingEar {
    heard: mpsc::UnboundedSender<String>,
    self_heard: Arc<AtomicUsize>,
}

struct RecordingEye {
    seen: mpsc::UnboundedSender<ImageData>,
}

struct RecordingMotion {
    felt: mpsc::UnboundedSender<BrowserMotion>,
}

#[async_trait]
impl Sensor<ImageData> for RecordingEye {
    async fn sense(&self, image: ImageData) {
        let _ = self.seen.send(image);
    }

    fn describe(&self) -> &'static str {
        "recording eye"
    }
}

#[async_trait]
impl Sensor<BrowserMotion> for RecordingMotion {
    async fn sense(&self, motion: BrowserMotion) {
        let _ = self.felt.send(motion);
    }

    fn describe(&self) -> &'static str {
        "recording motion"
    }
}

#[async_trait]
impl Ear for RecordingEar {
    async fn hear_self_say(&self, _text: &str) {
        self.self_heard.fetch_add(1, Ordering::SeqCst);
    }

    async fn hear_user_say(&self, text: &str) {
        let _ = self.heard.send(text.to_string());
    }
}

#[tokio::test]
async fn websocket_text_is_reported_to_ear() {
    let psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let (heard_tx, mut heard_rx) = mpsc::unbounded_channel();
    let ear = Arc::new(RecordingEar {
        heard: heard_tx,
        self_heard: Arc::new(AtomicUsize::new(0)),
    });
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    let motion = Arc::new(MotionSensor::new(psyche.input_sender()));
    let (bus, _user_rx) = EventBus::new();
    let bus = Arc::new(bus);
    let debug = psyche.debug_handle();
    let state = Body {
        asr: None,
        bus,
        ear,
        eye,
        geo,
        motion,
        conversation,
        connections: Arc::new(AtomicUsize::new(0)),
        system_prompt: Arc::new(Mutex::new(psyche.system_prompt())),
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
    let _ = socket.next().await.unwrap().unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "Text",
                "text": "hello pete"
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

    let heard = tokio::time::timeout(std::time::Duration::from_secs(1), heard_rx.recv())
        .await
        .expect("timed out waiting for websocket text to reach ear")
        .expect("ear channel closed");
    assert_eq!(heard, "hello pete");

    server.abort();
}

#[tokio::test]
async fn websocket_text_is_not_blocked_by_latest_eye_state() {
    let psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let (heard_tx, mut heard_rx) = mpsc::unbounded_channel();
    let ear = Arc::new(RecordingEar {
        heard: heard_tx,
        self_heard: Arc::new(AtomicUsize::new(0)),
    });
    let latest = Arc::new(std::sync::Mutex::new(None));
    let (latest_tx, _latest_rx) = tokio::sync::watch::channel(None);
    let eye = Arc::new(EyeSensor::latest_only(latest.clone(), latest_tx));
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    let motion = Arc::new(MotionSensor::new(psyche.input_sender()));
    let (bus, _user_rx) = EventBus::new();
    let bus = Arc::new(bus);
    let debug = psyche.debug_handle();
    let state = Body {
        asr: None,
        bus,
        ear,
        eye,
        geo,
        motion,
        conversation,
        connections: Arc::new(AtomicUsize::new(0)),
        system_prompt: Arc::new(Mutex::new(psyche.system_prompt())),
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
    let _ = socket.next().await.unwrap().unwrap();

    for data in ["data:image/jpeg;base64,b25l", "data:image/jpeg;base64,dHdv"] {
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::json!({
                    "type": "See",
                    "data": data
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
    }
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "Text",
                "data": { "text": "still listening" }
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

    let heard = tokio::time::timeout(std::time::Duration::from_secs(1), heard_rx.recv())
        .await
        .expect("timed out waiting for websocket text after image updates")
        .expect("ear channel closed");
    assert_eq!(heard, "still listening");
    assert_eq!(latest.lock().unwrap().as_ref().unwrap().base64, "dHdv");

    let entry = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let msg = socket.next().await.unwrap().unwrap();
            let tokio_tungstenite::tungstenite::Message::Text(text) = msg else {
                continue;
            };
            let payload: shared::WsPayload = serde_json::from_str(&text).unwrap();
            if let shared::WsPayload::ConversationEntry(entry) = payload {
                break entry;
            }
        }
    })
    .await
    .expect("timed out waiting for conversation entry after image updates");
    assert_eq!(entry.role, "user");
    assert_eq!(entry.content, "still listening");

    server.abort();
}

#[tokio::test]
async fn websocket_flat_see_is_reported_to_eye() {
    let psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let (heard_tx, _heard_rx) = mpsc::unbounded_channel();
    let ear = Arc::new(RecordingEar {
        heard: heard_tx,
        self_heard: Arc::new(AtomicUsize::new(0)),
    });
    let (seen_tx, mut seen_rx) = mpsc::unbounded_channel();
    let eye = Arc::new(RecordingEye { seen: seen_tx });
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    let (motion_tx, _motion_rx) = mpsc::unbounded_channel();
    let motion = Arc::new(RecordingMotion { felt: motion_tx });
    let (bus, _user_rx) = EventBus::new();
    let bus = Arc::new(bus);
    let debug = psyche.debug_handle();
    let state = Body {
        asr: None,
        bus,
        ear,
        eye,
        geo,
        motion,
        conversation,
        connections: Arc::new(AtomicUsize::new(0)),
        system_prompt: Arc::new(Mutex::new(psyche.system_prompt())),
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
    let _ = socket.next().await.unwrap().unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "See",
                "data": "data:image/jpeg;base64,Zm9v"
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

    let seen = tokio::time::timeout(std::time::Duration::from_secs(1), seen_rx.recv())
        .await
        .expect("timed out waiting for websocket image to reach eye")
        .expect("eye channel closed");
    assert_eq!(seen.mime, "image/jpeg");
    assert_eq!(seen.base64, "Zm9v");

    server.abort();
}

#[tokio::test]
async fn websocket_motion_is_reported_to_motion_sensor() {
    let psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let (heard_tx, _heard_rx) = mpsc::unbounded_channel();
    let ear = Arc::new(RecordingEar {
        heard: heard_tx,
        self_heard: Arc::new(AtomicUsize::new(0)),
    });
    let (seen_tx, _seen_rx) = mpsc::unbounded_channel();
    let eye = Arc::new(RecordingEye { seen: seen_tx });
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    let (motion_tx, mut motion_rx) = mpsc::unbounded_channel();
    let motion = Arc::new(RecordingMotion { felt: motion_tx });
    let (bus, _user_rx) = EventBus::new();
    let bus = Arc::new(bus);
    let debug = psyche.debug_handle();
    let state = Body {
        asr: None,
        bus,
        ear,
        eye,
        geo,
        motion,
        conversation,
        connections: Arc::new(AtomicUsize::new(0)),
        system_prompt: Arc::new(Mutex::new(psyche.system_prompt())),
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
    let _ = socket.next().await.unwrap().unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "Motion",
                "data": {
                    "acceleration": { "x": 1.0, "y": 2.0, "z": 3.0 },
                    "acceleration_including_gravity": { "x": 1.1, "y": 2.1, "z": 12.8 },
                    "rotation_rate": { "alpha": 4.0, "beta": 5.0, "gamma": 6.0 },
                    "orientation": { "alpha": 7.0, "beta": 8.0, "gamma": 9.0, "absolute": true },
                    "interval": 16.7
                },
                "at": "2026-05-06T12:00:00Z"
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

    let motion = tokio::time::timeout(std::time::Duration::from_secs(1), motion_rx.recv())
        .await
        .expect("timed out waiting for websocket motion to reach sensor")
        .expect("motion channel closed");
    assert_eq!(motion.acceleration.unwrap().x, Some(1.0));
    assert_eq!(motion.acceleration_including_gravity.unwrap().z, Some(12.8));
    assert_eq!(motion.rotation_rate.unwrap().gamma, Some(6.0));
    assert_eq!(motion.orientation.unwrap().absolute, Some(true));
    assert_eq!(motion.interval, Some(16.7));
    assert_eq!(motion.observed_at.as_deref(), Some("2026-05-06T12:00:00Z"));

    server.abort();
}
