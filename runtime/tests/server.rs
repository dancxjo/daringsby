use runtime::server::router;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use tokio::{runtime::Runtime, sync::{mpsc, Mutex}};
use std::sync::Arc;
use runtime::logger::SimpleLogger;

#[test]
fn root_serves_page() {
    let (tx, _rx) = mpsc::channel(1);
    let mood = Arc::new(Mutex::new(String::new()));
    let logger = SimpleLogger::init(log::LevelFilter::Info);
    let app = router(tx, mood, logger);
    Runtime::new().unwrap().block_on(async {
        let res = app
            .oneshot(Request::builder().uri("/").body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    });
}
