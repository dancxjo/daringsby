use crate::bus::{Event, EventBus};
use futures::{SinkExt, StreamExt};
use log::info;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
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

async fn handle_ws(bus: Arc<EventBus>, ws: WebSocket, peer: Option<SocketAddr>) {
    let (mut tx, mut rx_ws) = ws.split();
    let mut rx = bus.subscribe();
    if let Some(addr) = peer {
        info!("WebSocket client connected: {}", addr);
        bus.send(Event::Connected(addr));
    } else {
        info!("WebSocket client connected: unknown");
    }

    let forward = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let text = match event {
                Event::Log(line) | Event::Chat(line) => line,
                Event::Connected(addr) => format!("[connected {addr}]") ,
                Event::Disconnected(addr) => format!("[disconnected {addr}]") ,
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
                bus.send(Event::Chat(line));
            }
        }
    }

    let _ = forward.await;
    if let Some(addr) = peer {
        info!("WebSocket client disconnected: {}", addr);
        bus.send(Event::Disconnected(addr));
    } else {
        info!("WebSocket client disconnected: unknown");
    }
}

/// Start the webserver on the provided address.
pub async fn run(bus: Arc<EventBus>, addr: impl Into<SocketAddr>) {
    let html = warp::path::end().map(|| warp::reply::html(INDEX_HTML));
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::addr::remote())
        .map(move |ws: warp::ws::Ws, addr: Option<SocketAddr>| {
            let bus = bus.clone();
            ws.on_upgrade(move |socket| handle_ws(bus, socket, addr))
        });

    warp::serve(html.or(ws_route)).run(addr).await;
}
