use tokio_stream::StreamExt;
use voice::model::{registry::{ModelRegistry, ModelInfo, ServerInfo}, scheduler::ModelScheduler, Capability, Attribute, MockModelClient, ModelClient};

#[tokio::test]
async fn select_server_by_capability() {
    let mut registry = ModelRegistry::new();
    let server = ServerInfo::new("http://host1:11434")
        .with_model(ModelInfo::new("gemma", vec![Capability::Chat], vec![Attribute::Slow]));
    registry.add_server(server.clone());
    let scheduler = ModelScheduler::new(registry);
    let selected = scheduler.select(Capability::Chat, None).unwrap();
    assert_eq!(selected.address, server.address);
}

#[tokio::test]
async fn stream_from_mock() {
    let mock = MockModelClient::new(vec!["hi".into(), "there".into()], vec![1.0, 2.0]);
    let mut stream = mock.stream_chat("mock", "hello").await.unwrap();
    let mut out = Vec::new();
    while let Some(chunk) = stream.next().await {
        out.push(chunk.unwrap());
    }
    assert_eq!(out, vec!["hi", "there"]);
    let emb = mock.embed("mock", "foo").await.unwrap();
    assert_eq!(emb, vec![1.0, 2.0]);
}
