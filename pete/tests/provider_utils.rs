use httpmock::Method::POST;
use httpmock::MockServer;
use lingproc::Vectorizer;
use pete::ollama_provider_from_args;

#[tokio::test]
async fn builder_returns_provider() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/embed");
        then.status(200)
            .header("content-type", "application/json")
            .body("{\"embeddings\": [[1.0,2.0,3.0]]}");
    });

    let provider = ollama_provider_from_args(server.base_url().as_str(), "mistral").unwrap();
    let vec = provider.vectorize("hello").await.unwrap();
    mock.assert();
    assert_eq!(vec, vec![1.0, 2.0, 3.0]);
}
