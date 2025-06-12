use pete::web;
use psyche::{JoinScheduler, Psyche, bus::EventBus};
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::test::request;

#[tokio::test]
async fn index_serves() {
    let bus = Arc::new(EventBus::new());
    let psyche = Arc::new(Mutex::new(Psyche::new(|| JoinScheduler::default(), vec![])));
    let filter = web::routes(bus.clone(), psyche);
    let resp = request().method("GET").path("/").reply(&filter).await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn scheduler_reports_queue_and_memory() {
    use psyche::{Experience, Sensation, Sensor};
    let bus = Arc::new(EventBus::new());
    let psyche = Arc::new(Mutex::new(Psyche::new(|| JoinScheduler::default(), vec![])));
    {
        let mut p = psyche.lock().await;
        p.heart.quick.feel(Sensation::new(Experience::new("hi")));
    }
    let filter = web::routes(bus.clone(), psyche);
    let resp = request()
        .method("GET")
        .path("/scheduler")
        .reply(&filter)
        .await;
    assert_eq!(resp.status(), 200);
    let info: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(info["wits"][0]["queue_len"], 1);
    assert_eq!(info["wits"][0]["memory_len"], 0);
}

#[tokio::test]
async fn psyche_reports_beat() {
    let bus = Arc::new(EventBus::new());
    let psyche = Arc::new(Mutex::new(Psyche::new(|| JoinScheduler::default(), vec![])));
    let filter = web::routes(bus.clone(), psyche);
    let resp = request().method("GET").path("/psyche").reply(&filter).await;
    assert_eq!(resp.status(), 200);
    let info: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert!(info["beat"].as_u64().is_some());
}
