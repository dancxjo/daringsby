use async_trait::async_trait;
use futures_util::StreamExt;
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::{Sensation, Sensor};

/// Listens for WebSocket text frames and turns them into [`Sensation`]s.
pub struct WebSocketSensor {
    addr: SocketAddr,
}

impl WebSocketSensor {
    /// Create a sensor bound to the given socket address.
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

#[async_trait]
impl Sensor for WebSocketSensor {
    /// Accept connections and forward received messages as sensations.
    async fn run(&mut self, tx: mpsc::Sender<Sensation>) {
        let listener = TcpListener::bind(self.addr).await.expect("bind ws");
        while let Ok((stream, _)) = listener.accept().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                let ws_stream = accept_async(stream).await.expect("ws accept");
                let (_, mut read) = ws_stream.split();
                while let Some(Ok(Message::Text(text))) = read.next().await {
                    let _ = tx.send(Sensation::new("user message", Some(text))).await;
                }
            });
        }
    }
}
