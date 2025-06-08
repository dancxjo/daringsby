use crate::bus::{Event, global_bus};
use futures::{SinkExt, StreamExt};
use log::info;
use serde::Deserialize;
use std::net::SocketAddr;
use warp::{
    Filter,
    ws::{Message, WebSocket},
};

static INDEX_HTML: &str = include_str!("../static/index.html");

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientMessage {
    Chat { line: String },
}

async fn handle_ws(ws: WebSocket) {
    let (mut tx, mut rx_ws) = ws.split();
    let mut rx = global_bus().subscribe();
    info!("WebSocket client connected");

    let forward = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let text = match event {
                Event::Log(line) | Event::Chat(line) => line,
            };
            if tx.send(Message::text(text)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = rx_ws.next().await {
        if msg.is_text() {
            if let Ok(ClientMessage::Chat { line }) =
                serde_json::from_str::<ClientMessage>(msg.to_str().unwrap_or(""))
            {
                global_bus().send(Event::Chat(line));
            }
        }
    }

    let _ = forward.await;
    info!("WebSocket client disconnected");
}

/// Start the webserver on the provided address.
pub async fn run(addr: impl Into<SocketAddr>) {
    let html = warp::path::end().map(|| warp::reply::html(INDEX_HTML));
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(handle_ws));

    warp::serve(html.or(ws_route)).run(addr).await;
}
