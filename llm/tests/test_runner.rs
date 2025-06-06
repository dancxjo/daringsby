use llm::{runner::stream_first_sentence, OllamaClient};
use tokio_stream::StreamExt;

mod mock_server;
use mock_server::spawn_mock_server;

#[tokio::test]
async fn capture_first_sentence() {
    let (url, shutdown) = spawn_mock_server(vec!["Hello ", "world.", " More."]).await;
    let client = OllamaClient::new(&url);
    let (tokens, sentence) = stream_first_sentence(&client, "gemma3:27b", "test")
        .await
        .unwrap();
    assert_eq!(
        tokens,
        vec![
            "Hello ".to_string(),
            "world.".to_string(),
            " More.".to_string()
        ]
    );
    assert_eq!(sentence, "Hello world. ");
    let _ = shutdown.send(()).await;
}
