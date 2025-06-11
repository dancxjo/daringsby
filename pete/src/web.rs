use futures_util::{SinkExt, StreamExt, future};
use serde::{Deserialize, Serialize};
use serde_json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{Filter, ws::Message};

use psyche::{
    Psyche, Scheduler,
    bus::{Event, EventBus},
};

static INDEX_HTML: &str = include_str!("../../psyche/static/index.html");

#[derive(Serialize)]
struct WitStaticInfo {
    name: Option<String>,
    interval_ms: u64,
}

#[derive(Serialize)]
struct PsycheInfo {
    instant: Option<String>,
    moment: Option<String>,
    context: Option<String>,
    wits: Vec<WitStaticInfo>,
}

#[derive(Serialize)]
struct WitRuntimeInfo {
    name: Option<String>,
    queue_len: usize,
    due_ms: u64,
    last: Option<String>,
}

#[derive(Serialize)]
struct SchedulerInfo {
    wits: Vec<WitRuntimeInfo>,
}

#[derive(Deserialize)]
struct ChatMsg {
    #[serde(rename = "type")]
    kind: String,
    line: String,
}

fn wit_static<S>(w: &psyche::Wit<S>) -> WitStaticInfo
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    WitStaticInfo {
        name: w.name.clone(),
        interval_ms: w.interval.as_millis() as u64,
    }
}

fn psyche_info<S>(p: &Psyche<S>) -> PsycheInfo
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    PsycheInfo {
        instant: p.heart.instant.as_ref().map(|e| e.how.clone()),
        moment: p.heart.moment.as_ref().map(|e| e.how.clone()),
        context: p.heart.context.clone(),
        wits: vec![
            wit_static(&p.heart.quick),
            wit_static(&p.heart.combobulator),
            wit_static(&p.heart.contextualizer),
        ],
    }
}

fn wit_runtime<S>(w: &psyche::Wit<S>) -> WitRuntimeInfo
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    let due = w.due_ms();
    let last = w.memory.all().last().map(|s| s.what.clone().into());
    WitRuntimeInfo {
        name: w.name.clone(),
        queue_len: w.queue_len(),
        due_ms: due,
        last,
    }
}

fn scheduler_info<S>(p: &Psyche<S>) -> SchedulerInfo
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    SchedulerInfo {
        wits: vec![
            wit_runtime(&p.heart.quick),
            wit_runtime(&p.heart.combobulator),
            wit_runtime(&p.heart.contextualizer),
        ],
    }
}

async fn handle_ws(ws: warp::ws::WebSocket, addr: Option<SocketAddr>, bus: Arc<EventBus>) {
    if let Some(a) = addr {
        bus.send(Event::Connected(a));
    }
    let (mut sender, mut receiver) = ws.split();
    let mut bus_rx = bus.subscribe();
    let send_task = tokio::spawn(async move {
        while let Ok(evt) = bus_rx.recv().await {
            let line = match evt {
                Event::Log(l) => l,
                Event::Chat(l) => format!("chat: {l}"),
                Event::Connected(a) => format!("connected {a}"),
                Event::Disconnected(a) => format!("disconnected {a}"),
            };
            if sender.send(Message::text(line)).await.is_err() {
                break;
            }
        }
    });
    let bus_send = bus.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if msg.is_text() {
                if let Ok(chat) = serde_json::from_str::<ChatMsg>(msg.to_str().unwrap()) {
                    if chat.kind == "chat" {
                        bus_send.send(Event::Chat(chat.line));
                    }
                }
            }
        }
    });
    let _ = future::join(send_task, recv_task).await;
    if let Some(a) = addr {
        bus.send(Event::Disconnected(a));
    }
}

pub fn routes<S>(
    bus: Arc<EventBus>,
    psyche: Arc<Mutex<Psyche<S>>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
where
    S: Scheduler + Send + 'static,
    S::Output: Clone + Into<String> + Send + 'static,
{
    let index = warp::path::end().map(|| warp::reply::html(INDEX_HTML));

    let bus_ws = bus.clone();
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::addr::remote())
        .map(move |ws: warp::ws::Ws, addr| {
            let bus = bus_ws.clone();
            ws.on_upgrade(move |socket| handle_ws(socket, addr, bus))
        });

    let psyche_state = psyche.clone();
    let psyche_route = warp::path("psyche").and_then(move || {
        let psyche = psyche_state.clone();
        async move {
            let p = psyche.lock().await;
            let info = psyche_info(&*p);
            Ok::<_, warp::Rejection>(warp::reply::json(&info))
        }
    });

    let psyche_sched = psyche.clone();
    let sched_route = warp::path("scheduler").and_then(move || {
        let psyche = psyche_sched.clone();
        async move {
            let p = psyche.lock().await;
            let info = scheduler_info(&*p);
            Ok::<_, warp::Rejection>(warp::reply::json(&info))
        }
    });

    index.or(ws_route).or(psyche_route).or(sched_route)
}

pub async fn serve<S>(bus: Arc<EventBus>, psyche: Arc<Mutex<Psyche<S>>>) -> anyhow::Result<()>
where
    S: Scheduler + Send + 'static,
    S::Output: Clone + Into<String> + Send + 'static,
{
    warp::serve(routes(bus, psyche))
        .run(([0, 0, 0, 0], 8080))
        .await;
    Ok(())
}
