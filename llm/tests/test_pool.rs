use tokio_stream::StreamExt;
use std::sync::Arc;

use llm::{
    LLMClientPool, LLMModel, LLMServer, LLMCapability, LLMAttribute, OllamaClient, LLMError,
};
use llm::LinguisticTask;

mod mock_server;
use mock_server::spawn_mock_server;

#[tokio::test]
async fn capability_and_tag_lookup() {
    let client = Arc::new(OllamaClient::new("http://localhost:1234"));
    let server = LLMServer::new(client)
        .with_attribute(LLMAttribute::Fast)
        .with_model(LLMModel::new("gemma3:27b", vec![LLMCapability::Chat, LLMCapability::Vision]));

    let mut pool = LLMClientPool::new();
    pool.add_server(server);

    let caps = pool.model_capabilities("gemma3:27b").unwrap();
    assert_eq!(caps, vec![LLMCapability::Chat, LLMCapability::Vision]);
    assert!(pool.has_attribute("gemma3:27b", LLMAttribute::Fast));
}

#[tokio::test]
async fn stream_chat_from_mock() {
    let (url, shutdown) = spawn_mock_server(vec!["hi", "there"]).await;
    let client = Arc::new(OllamaClient::new(&url));
    let server = LLMServer::new(client).with_model(LLMModel::new("gemma3:27b", vec![LLMCapability::Chat]));
    let mut pool = LLMClientPool::new();
    pool.add_server(server);

    let mut stream = pool.stream_chat("gemma3:27b", "hello").await.unwrap();
    let mut out = Vec::new();
    while let Some(c) = stream.next().await {
        out.push(c.unwrap());
    }
    assert_eq!(out, vec!["hi".to_string(), "there".to_string()]);
    let _ = shutdown.send(()).await;
}

#[tokio::test]
async fn choose_model_matches_caps() {
    let client = Arc::new(OllamaClient::new("http://localhost:1234"));
    let server = LLMServer::new(client)
        .with_attribute(LLMAttribute::Fast)
        .with_model(LLMModel::new("gemma3:27b", vec![LLMCapability::Chat]));
    let mut pool = LLMClientPool::new();
    pool.add_server(server);

    let task = LinguisticTask::new("ping", vec![LLMCapability::Chat])
        .prefer_attribute(LLMAttribute::Fast);
    let model = pool.choose_model(&task.capabilities, task.prefer).unwrap();
    assert_eq!(model, "gemma3:27b");
}

#[tokio::test]
async fn round_robin_across_hosts() {
    let (url1, shutdown1) = spawn_mock_server(vec!["one"]).await;
    let (url2, shutdown2) = spawn_mock_server(vec!["two"]).await;
    let client1 = Arc::new(OllamaClient::new(&url1));
    let client2 = Arc::new(OllamaClient::new(&url2));
    let server1 = LLMServer::new(client1).with_model(LLMModel::new(
        "gemma3:27b",
        vec![LLMCapability::Chat],
    ));
    let server2 = LLMServer::new(client2).with_model(LLMModel::new(
        "gemma3:27b",
        vec![LLMCapability::Chat],
    ));
    let mut pool = LLMClientPool::new();
    pool.add_server(server1);
    pool.add_server(server2);

    let mut first = pool.stream_chat("gemma3:27b", "hello").await.unwrap();
    let out1: Vec<_> = first.next().await.map(|r| r.unwrap()).into_iter().collect();
    let mut second = pool.stream_chat("gemma3:27b", "hello").await.unwrap();
    let out2: Vec<_> = second.next().await.map(|r| r.unwrap()).into_iter().collect();
    assert_eq!(out1, vec!["one".to_string()]);
    assert_eq!(out2, vec!["two".to_string()]);
    let _ = shutdown1.send(()).await;
    let _ = shutdown2.send(()).await;
}
use mock_server::spawn_delayed_mock_server;

#[tokio::test]
async fn prefers_fast_server_after_profiling() {
    let (fast_url, shutdown_fast) = spawn_delayed_mock_server(vec!["fast", "fast"], 10).await;
    let (slow_url, shutdown_slow) = spawn_delayed_mock_server(vec!["slow", "slow"], 100).await;
    let client1 = Arc::new(OllamaClient::new(&fast_url));
    let client2 = Arc::new(OllamaClient::new(&slow_url));
    let server1 = LLMServer::new(client1)
        .with_attribute(LLMAttribute::Fast)
        .with_model(LLMModel::new("gemma3:27b", vec![LLMCapability::Chat]));
    let server2 = LLMServer::new(client2)
        .with_attribute(LLMAttribute::Slow)
        .with_model(LLMModel::new("gemma3:27b", vec![LLMCapability::Chat]));
    let mut pool = LLMClientPool::new();
    pool.add_server(server1);
    pool.add_server(server2);

    let mut s1 = pool
        .run_task(&LinguisticTask::new("hi", vec![LLMCapability::Chat]).prefer_attribute(LLMAttribute::Fast))
        .await
        .unwrap();
    let _ = s1.next().await;
    let mut s2 = pool
        .run_task(&LinguisticTask::new("hi", vec![LLMCapability::Chat]).prefer_attribute(LLMAttribute::Slow))
        .await
        .unwrap();
    let _ = s2.next().await;

    let mut result = pool.stream_chat("gemma3:27b", "hello").await.unwrap();
    let token = result.next().await.unwrap().unwrap();
    assert_eq!(token, "fast".to_string());
    let _ = shutdown_fast.send(()).await;
    let _ = shutdown_slow.send(()).await;
}

#[tokio::test]
async fn droppable_task_is_skipped_when_busy() {
    let (url, shutdown) = spawn_delayed_mock_server(vec!["one"], 100).await;
    let client = Arc::new(OllamaClient::new(&url));
    let server = LLMServer::new(client).with_model(LLMModel::new(
        "gemma3:27b",
        vec![LLMCapability::Chat],
    ));
    let mut pool = LLMClientPool::new();
    pool.add_server(server);

    let mut s1 = pool.stream_chat("gemma3:27b", "a").await.unwrap();
    let res = pool
        .run_task(
            &LinguisticTask::new("b", vec![LLMCapability::Chat]).droppable(true),
        )
        .await;
    assert!(matches!(res, Err(LLMError::QueueFull)));
    let _ = s1.next().await;
    let _ = shutdown.send(()).await;
}
