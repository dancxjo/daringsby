use crate::bus::{Event, EventBus};
use crate::{ProcessorScheduler, Psyche, Scheduler};
use futures::{SinkExt, StreamExt};
use lingproc::OllamaProcessor;
use log::info;
use serde::Deserialize;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
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
                Event::Connected(addr) => format!("[connected {addr}]"),
                Event::Disconnected(addr) => format!("[disconnected {addr}]"),
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

#[derive(Serialize)]
struct WitInfo {
    name: Option<String>,
    interval_ms: u64,
    memory: usize,
}

#[derive(Serialize)]
struct PsycheInfo {
    wits: Vec<WitInfo>,
    sensors: Vec<String>,
}

#[derive(Serialize)]
struct SchedulerEntry {
    index: usize,
    name: Option<String>,
    scheduler: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capabilities: Option<Vec<String>>,
    queue_len: usize,
    due_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct SchedulerInfo {
    wits: Vec<SchedulerEntry>,
}

async fn psyche_handler<S>(
    psyche: Arc<Mutex<Psyche<S>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    S: Scheduler + Send + Sync,
    S::Output: Serialize + Clone + Into<String>,
{
    use std::any::type_name_of_val;
    let psyche = psyche.lock().await;
    let wits = psyche
        .heart
        .wits
        .iter()
        .map(|w| WitInfo {
            name: w.name.clone(),
            interval_ms: w.interval.as_millis() as u64,
            memory: w.memory.all().len(),
        })
        .collect();
    let sensors = psyche
        .sensors
        .iter()
        .map(|s| type_name_of_val(&**s).to_string())
        .collect();
    Ok(warp::reply::json(&PsycheInfo { wits, sensors }))
}

async fn scheduler_handler<S>(
    psyche: Arc<Mutex<Psyche<S>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    S: Scheduler + Send + Sync + 'static,
    S::Output: Serialize + Clone + Into<String>,
{
    use std::any::{Any, type_name};
    let psyche = psyche.lock().await;
    let sched_type = type_name::<S>().to_string();
    let wits = psyche
        .heart
        .wits
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let proc_sched =
                (&w.scheduler as &dyn Any).downcast_ref::<ProcessorScheduler<OllamaProcessor>>();
            let model = proc_sched.map(|ps| ps.processor.model.clone());
            let capabilities = proc_sched.map(|ps| {
                ps.capabilities()
                    .into_iter()
                    .map(|c| format!("{:?}", c))
                    .collect::<Vec<_>>()
            });
            let last = w
                .memory
                .all()
                .last()
                .map(|s| serde_json::to_value(&s.what).unwrap());
            SchedulerEntry {
                index: i,
                name: w.name.clone(),
                scheduler: sched_type.clone(),
                model,
                capabilities,
                queue_len: w.queue.len(),
                due_ms: w.interval.saturating_sub(w.last_tick.elapsed()).as_millis() as u64,
                last,
            }
        })
        .collect();
    Ok(warp::reply::json(&SchedulerInfo { wits }))
}

async fn wit_handler<S>(
    name: String,
    psyche: Arc<Mutex<Psyche<S>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    S: Scheduler + Send + Sync,
    S::Output: Serialize + Clone + Into<String>,
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
pub async fn run_with_psyche<S>(
    bus: Arc<EventBus>,
    psyche: Arc<Mutex<Psyche<S>>>,
    addr: impl Into<SocketAddr>,
) where
    S: Scheduler + Send + Sync + 'static,
    S::Output: Serialize + Clone + Into<String> + Send + Sync + 'static,
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
        .and(psyche_filter.clone())
        .and_then(wit_handler::<S>);

    let psyche_route = warp::path("psyche")
        .and(psyche_filter.clone())
        .and_then(psyche_handler::<S>);

    let scheduler_route = warp::path("scheduler")
        .and(psyche_filter)
        .and_then(scheduler_handler::<S>);

    warp::serve(
        html.or(ws_route)
            .or(wit_route)
            .or(psyche_route)
            .or(scheduler_route),
    )
    .run(addr)
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Experience, Heart, JoinScheduler, Sensation, Wit};
    use serde_json::Value;
    use warp::Reply;

    struct Echo;

    impl crate::Sensor for Echo {
        type Input = String;
        fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
            Some(Experience::new(s.what))
        }
    }

    #[tokio::test]
    async fn wit_endpoint_returns_memory() {
        let heart = Heart::new(vec![Wit::new(JoinScheduler::default())]);
        let psyche = Arc::new(Mutex::new(Psyche::new(heart, vec![])));

        {
            let mut p = psyche.lock().await;
            p.heart.wits[0]
                .memory
                .remember(Sensation::new("hello".to_string()));
        }

        let resp = wit_handler::<JoinScheduler>("0".into(), psyche.clone())
            .await
            .unwrap();
        let body = resp.into_response();
        assert_eq!(body.status(), 200);
        let bytes = warp::hyper::body::to_bytes(body.into_body()).await.unwrap();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val.as_array().unwrap()[0]["what"], "hello");
    }

    #[tokio::test]
    async fn psyche_endpoint_lists_wits() {
        let heart = Heart::new(vec![Wit::with_config(
            ProcessorScheduler::new(OllamaProcessor::new("model")),
            Some("w1".into()),
            std::time::Duration::from_secs(0),
        )]);
        let psyche = Arc::new(Mutex::new(Psyche::new(
            heart,
            vec![Box::new(crate::sensors::ChatSensor::default())],
        )));

        let resp = psyche_handler::<ProcessorScheduler<OllamaProcessor>>(psyche.clone())
            .await
            .unwrap();
        let body = resp.into_response();
        assert_eq!(body.status(), 200);
        let bytes = warp::hyper::body::to_bytes(body.into_body()).await.unwrap();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["wits"][0]["name"], "w1");
        assert_eq!(val["sensors"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn scheduler_endpoint_reports_model() {
        let heart = Heart::new(vec![Wit::with_config(
            ProcessorScheduler::new(OllamaProcessor::new("llama-test")),
            None,
            std::time::Duration::from_secs(0),
        )]);
        let psyche = Arc::new(Mutex::new(Psyche::new(heart, vec![])));

        let resp = scheduler_handler::<ProcessorScheduler<OllamaProcessor>>(psyche.clone())
            .await
            .unwrap();
        let body = resp.into_response();
        assert_eq!(body.status(), 200);
        let bytes = warp::hyper::body::to_bytes(body.into_body()).await.unwrap();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["wits"][0]["model"], "llama-test");
    }

    #[tokio::test]
    async fn scheduler_endpoint_reports_queue_and_due() {
        let heart = Heart::new(vec![Wit::with_config(
            JoinScheduler::default(),
            Some("q".into()),
            std::time::Duration::from_millis(100),
        )]);
        let psyche = Arc::new(Mutex::new(Psyche::new(heart, vec![])));

        {
            let mut p = psyche.lock().await;
            p.heart.wits[0].push(Experience::new("hi"));
            p.heart.wits[0].last_tick =
                std::time::Instant::now() - std::time::Duration::from_millis(50);
        }

        let resp = scheduler_handler::<JoinScheduler>(psyche.clone())
            .await
            .unwrap();
        let body = resp.into_response();
        assert_eq!(body.status(), 200);
        let bytes = warp::hyper::body::to_bytes(body.into_body()).await.unwrap();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["wits"][0]["queue_len"], 1);
        let due = val["wits"][0]["due_ms"].as_u64().unwrap();
        assert!(due <= 100);
    }

    #[tokio::test]
    async fn scheduler_endpoint_reports_last_memory() {
        let heart = Heart::new(vec![Wit::with_config(
            JoinScheduler::default(),
            None,
            std::time::Duration::from_secs(0),
        )]);
        let psyche = Arc::new(Mutex::new(Psyche::new(heart, vec![])));

        {
            let mut p = psyche.lock().await;
            p.heart.wits[0].push(Experience::new("hello"));
            let _ = p.heart.tick();
        }

        let resp = scheduler_handler::<JoinScheduler>(psyche.clone())
            .await
            .unwrap();
        let body = resp.into_response();
        assert_eq!(body.status(), 200);
        let bytes = warp::hyper::body::to_bytes(body.into_body()).await.unwrap();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["wits"][0]["last"], "hello");
    }
}
