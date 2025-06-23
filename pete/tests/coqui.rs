#![cfg(feature = "tts")]
use futures::StreamExt;
use httpmock::{Method::GET, MockServer};
use pete::{CoquiTts, Tts};

#[tokio::test]
async fn coqui_url_has_required_params() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/tts")
                .query_param("text", "hello")
                .query_param("speaker_id", "p1")
                .query_param("style_wav", "")
                .query_param("language_id", "en");
            then.status(200).body("abcd");
        })
        .await;

    let tts = CoquiTts::new(server.url("/api/tts"), Some("p1".into()), Some("en".into()));
    let mut stream = tts.stream_wav("hello").await.unwrap();
    while let Some(chunk) = stream.next().await {
        chunk.unwrap();
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn coqui_defaults_voice() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/tts")
                .query_param("text", "hi")
                .query_param("speaker_id", "p123")
                .query_param("style_wav", "")
                .query_param("language_id", "");
            then.status(200).body("abcd");
        })
        .await;

    let tts = CoquiTts::new(server.url("/api/tts"), None, None);
    let mut stream = tts.stream_wav("hi").await.unwrap();
    while let Some(chunk) = stream.next().await {
        chunk.unwrap();
    }
    mock.assert_async().await;
}
