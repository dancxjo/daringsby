use pete::index;

#[tokio::test]
async fn serves_index_html() {
    let resp = index().await;
    assert!(resp.0.contains("ws://localhost:3000/ws"));
    assert!(resp.0.contains("WS:"));
    assert_eq!(resp.0.matches("new WebSocket").count(), 1);
}
