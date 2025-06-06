use std::env;

use tts::speak_from_llm;

mod mock_tts_server;
mod mock_llm_server;
use mock_tts_server::spawn_mock_tts;
use mock_llm_server::spawn_mock_server;

#[tokio::test]
async fn speak_from_llm_pipeline() {
    let (llm_url, llm_shutdown) = spawn_mock_server(vec!["Hello world! 😊"]).await;
    let (tts_url, tts_shutdown) = spawn_mock_tts(b"wav").await;
    env::set_var("OLLAMA_URL", &llm_url);
    env::set_var("COQUI_URL", &tts_url);
    env::set_var("SPEAKER", "test");

    let bytes = speak_from_llm("hi").await.unwrap();
    assert_eq!(bytes, b"wav");

    let _ = llm_shutdown.send(()).await;
    let _ = tts_shutdown.send(()).await;
}
