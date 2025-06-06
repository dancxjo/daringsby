use warp::Filter;
use tokio::sync::mpsc;

pub async fn spawn_mock_tts(response: &'static [u8]) -> (String, mpsc::Sender<()>) {
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
    let data = response.to_vec();
    let route = warp::post()
        .and(warp::path("api").and(warp::path("tts")))
        .map(move || warp::reply::Response::new(data.clone().into()));

    let (addr, server) = warp::serve(route).bind_with_graceful_shutdown(([127,0,0,1],0), async move {
        shutdown_rx.recv().await;
    });
    tokio::spawn(server);
    let url = format!("http://{}", addr);
    (url, shutdown_tx)
}
