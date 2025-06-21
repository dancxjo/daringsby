use axum::extract::Path;
use pete::wit_debug_page;

#[tokio::test]
async fn serves_wit_debug_html() {
    let resp = wit_debug_page(Path("will".to_string())).await;
    let body = resp.0;
    assert!(body.contains("Debug for"));
}
