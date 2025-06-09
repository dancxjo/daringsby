use crate::bus::{Event, EventBus};
use crate::{Psyche, Sensor, Scheduler};
use tokio::sync::Mutex;
use serde::Serialize;
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

async fn wit_handler<S, P>(
    name: String,
    psyche: Arc<Mutex<Psyche<S, P>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    S: Scheduler + Send + Sync,
    P: Sensor<Input = S::Output> + Send + Sync,
    S::Output: Serialize + Clone,
{
    let psyche = psyche.lock().await;
    let idx = name.parse::<usize>().ok();
    let wit_opt = if let Some(i) = idx {
        psyche.heart.wits.get(i)
    } else {
        psyche
            .heart
            .wits
            .iter()
            .find(|w| w.name.as_deref() == Some(&name))
    };
    if let Some(wit) = wit_opt {
        return Ok(warp::reply::json(&wit.memory.all()));
    }
    Err(warp::reject::not_found())
}

/// Start the webserver with access to a [`Psyche`].
pub async fn run_with_psyche<S, P>(
    bus: Arc<EventBus>,
    psyche: Arc<Mutex<Psyche<S, P>>>,
    addr: impl Into<SocketAddr>,
) where
    S: Scheduler + Send + Sync + 'static,
    P: Sensor<Input = S::Output> + Send + Sync + 'static,
    S::Output: Serialize + Clone + Send + Sync + 'static,
{

    let html = warp::path::end().map(|| warp::reply::html(INDEX_HTML));
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::addr::remote())
        .map(move |ws: warp::ws::Ws, addr: Option<SocketAddr>| {
            let bus = bus.clone();
            ws.on_upgrade(move |socket| handle_ws(bus, socket, addr))
        });

    let psyche_filter = warp::any().map(move || psyche.clone());
    let wit_route = warp::path!("wit" / String)
        .and(psyche_filter)
        .and_then(wit_handler::<S, P>);

    warp::serve(html.or(ws_route).or(wit_route)).run(addr).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use warp::Reply;
    use crate::{Experience, JoinScheduler, Sensation, Heart, Wit};

    struct Echo;

    impl crate::Sensor for Echo {
        type Input = String;
        fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
            Some(Experience::new(s.what))
        }
    }

    #[tokio::test]
    async fn wit_endpoint_returns_memory() {
        let heart = Heart::new(vec![Wit::new(JoinScheduler::default(), Echo)]);
        let psyche = Arc::new(Mutex::new(Psyche::new(heart, vec![])));

        {
            let mut p = psyche.lock().await;
            p.heart.wits[0].memory.remember(Sensation::new("hello".to_string()));
        }

        let resp = wit_handler::<JoinScheduler, Echo>("0".into(), psyche.clone())
            .await
            .unwrap();
        let body = resp.into_response();
        assert_eq!(body.status(), 200);
        let bytes = warp::hyper::body::to_bytes(body.into_body()).await.unwrap();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val.as_array().unwrap()[0]["what"], "hello");
    }
}
