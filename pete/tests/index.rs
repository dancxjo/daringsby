use pete::index;

#[tokio::test]
async fn serves_status_text() {
    let resp = index().await;
    assert!(resp.0.contains("WebSocket server"));
    assert!(resp.0.contains("/ws"));
}
