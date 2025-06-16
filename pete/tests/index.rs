use pete::index;

#[tokio::test]
async fn serves_index_html() {
    let resp = index().await;
    assert!(resp.0.contains("ws://localhost:3000/ws"));
    assert!(resp.0.contains("ws://localhost:3000/log"));
    assert!(resp.0.contains("WS:"));
    assert_eq!(resp.0.matches("new WebSocket").count(), 2);
    assert!(
        resp.0
            .contains("this.log[this.log.length - 1].text += text")
    );
    assert!(resp.0.contains("audioQueue"));
}
