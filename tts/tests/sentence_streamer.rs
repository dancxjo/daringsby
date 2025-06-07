use tts::sentence_streamer::SentenceStreamer;

#[tokio::test]
async fn enqueue_sends_text() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let streamer = SentenceStreamer::new(move |s| {
        let _ = tx.try_send(s);
        Ok(Vec::new())
    });
    streamer.enqueue("hello".into()).await.unwrap();
    let got = rx.recv().await.unwrap();
    assert_eq!(got, "hello");
}
