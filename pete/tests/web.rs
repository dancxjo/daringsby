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
