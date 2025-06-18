use httpmock::Method::POST;
use httpmock::MockServer;
use lingproc::{Vectorizer, provider::OllamaProvider};

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
