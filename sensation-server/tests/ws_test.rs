use tokio_tungstenite::connect_async;
use tokio::net::TcpListener;
use axum::{routing::get, Router};
use futures_util::{SinkExt, StreamExt};
use tokio::time::{timeout, Duration};
use sensation_server::ws_handler;

#[tokio::test]
async fn websocket_echo_sensation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/ws", get(ws_handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{}/ws", addr);
    let (mut ws, _) = connect_async(url).await.unwrap();
    let msg = r#"{\"sensor_type\":\"geolocation\",\"lat\":1.0,\"lon\":2.0}"#;
    ws.send(tokio_tungstenite::tungstenite::Message::Text(msg.into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    ws.close(None).await.unwrap();
}
