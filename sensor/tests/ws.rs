use sensor::{ws::WebSocketSensor, Sensor};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use futures_util::SinkExt;
use url::Url;
use std::net::SocketAddr;

#[tokio::test]
async fn ws_emits_message() {
    let addr: SocketAddr = "127.0.0.1:32154".parse().unwrap();
    let (tx, mut rx) = mpsc::channel(1);
    let mut ws = WebSocketSensor::new(addr);
    tokio::spawn(async move { ws.run(tx).await; });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = Url::parse(&format!("ws://{}", addr)).unwrap();
    let (mut stream, _) = connect_async(url).await.unwrap();
    stream.send(tokio_tungstenite::tungstenite::Message::Text("hi".into())).await.unwrap();
    let s = rx.recv().await.unwrap();
    assert_eq!(s.what.as_deref(), Some("hi"));
}
