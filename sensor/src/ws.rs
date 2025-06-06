use async_trait::async_trait;
use tokio::{net::TcpListener, sync::mpsc};
use futures_util::StreamExt;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use std::net::SocketAddr;

use crate::{Sensation, Sensor};

pub struct WebSocketSensor {
    addr: SocketAddr,
}

impl WebSocketSensor {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

#[async_trait]
impl Sensor for WebSocketSensor {
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
