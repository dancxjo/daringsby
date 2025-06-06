use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use warp::Filter;
use tokio::sync::mpsc;
use serde_json::json;

pub async fn spawn_mock_server(responses: Vec<&'static str>) -> (String, mpsc::Sender<()>) {
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
    let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
    let shared = warp::any().map(move || queue.clone());

    let route = warp::post()
        .and(warp::path("api").and(warp::path("generate")))
        .and(shared)
        .map(|queue: Arc<Mutex<VecDeque<&'static str>>>| {
            let (mut tx, body) = warp::hyper::Body::channel();
            tokio::spawn(async move {
                loop {
                    let item = { queue.lock().unwrap().pop_front() };
                    if let Some(r) = item {
                        let done = queue.lock().unwrap().is_empty();
                        let obj = json!({
                            "model": "gemma3:27b",
                            "created_at": "now",
                            "response": r,
                            "done": done
                        });
                        let line = serde_json::to_string(&obj).unwrap() + "\n";
                        if tx.send_data(line.into()).await.is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            });
            warp::reply::Response::new(body)
        });

    let (addr, server) = warp::serve(route).bind_with_graceful_shutdown(([127,0,0,1],0), async move {
        shutdown_rx.recv().await;
    });
    tokio::spawn(server);
    let url = format!("http://{}", addr);
    (url, shutdown_tx)
}
