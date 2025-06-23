use httpmock::Method::POST;
use httpmock::MockServer;
use httpmock::prelude::HttpMockRequest;
use lingproc::{Doer, ImageData, LlmInstruction, Vectorizer, provider::OllamaProvider};

#[tokio::test]
async fn vectorize_returns_floats() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/embed");
        then.status(200)
            .header("content-type", "application/json")
            .body("{\"embeddings\": [[1.0,2.0,3.0]]}");
    });

    let provider = OllamaProvider::new(server.base_url(), "mistral").unwrap();
    let vec = provider.vectorize("hello").await.unwrap();
    mock.assert();
    assert_eq!(vec, vec![1.0, 2.0, 3.0]);
}

#[tokio::test]
async fn follow_includes_images() {
    let server = MockServer::start_async().await;
    fn body_contains_abcd(req: &HttpMockRequest) -> bool {
        req.body
            .as_ref()
            .map(|b| std::str::from_utf8(b).unwrap_or_default().contains("abcd"))
            .unwrap_or(false)
    }

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/chat")
            .matches(body_contains_abcd);
        then.status(200)
            .header("content-type", "application/json")
            .body("{\"model\":\"mistral\",\"created_at\":\"now\",\"message\":{\"role\":\"assistant\",\"content\":\"ok\"},\"done\":true}");
    });

    let provider = OllamaProvider::new(server.base_url(), "mistral").unwrap();
    let res = provider
        .follow(LlmInstruction {
            command: "look".into(),
            images: vec![ImageData {
                mime: "image/jpeg".into(),
                base64: "abcd".into(),
            }],
        })
        .await
        .unwrap();
    mock.assert();
    assert_eq!(res, "ok");
}
