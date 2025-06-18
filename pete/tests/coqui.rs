#![cfg(feature = "tts")]
use futures::StreamExt;
use httpmock::{Method::GET, MockServer};
use pete::CoquiTts;

#[tokio::test]
async fn coqui_url_has_required_params() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/tts")
                .query_param("text", "hello")
                .query_param("speaker_id", "p1")
                .query_param("language_id", "en")
                .matches(|req| {
                    req.query_params
                        .as_ref()
                        .map_or(true, |qp| !qp.iter().any(|(k, _)| k == "style_wav"))
                });
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
