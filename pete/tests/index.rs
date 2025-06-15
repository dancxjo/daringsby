use pete::index;

#[tokio::test]
async fn serves_index_html() {
    let resp = index().await;
    assert!(resp.0.contains("Chat with Pete"));
}
