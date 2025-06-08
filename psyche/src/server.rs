use crate::bus::{Event, global_bus};
use futures::SinkExt;
use log::info;
use std::net::SocketAddr;
use warp::{
    Filter,
    ws::{Message, WebSocket},
};

static INDEX_HTML: &str = include_str!("../static/index.html");

async fn handle_ws(mut ws: WebSocket) {
    let mut rx = global_bus().subscribe();
    info!("WebSocket client connected");
    while let Ok(event) = rx.recv().await {
        match event {
            Event::Log(line) => {
                if ws.send(Message::text(line)).await.is_err() {
                    break;
                }
            }
        }
    }
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
