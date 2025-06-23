use futures::StreamExt;
use lingproc::{segment_text_into_sentences, sentence_stream, stream_sentence_chunks};
use tokio_stream::iter;

#[tokio::test]
async fn segment_text_into_sentences_splits() {
    let sentences = segment_text_into_sentences("Hello world. How are you?");
    assert_eq!(
        sentences,
        vec!["Hello world.".to_string(), "How are you?".to_string()]
    );
}

#[tokio::test]
async fn stream_sentence_chunks_emits_sentences() {
    let chunks = vec!["Hello world. ", "How are you?"]
        .into_iter()
        .map(String::from);
    let mut stream = Box::pin(stream_sentence_chunks(iter(chunks)));
    assert_eq!(stream.next().await.unwrap(), "Hello world.");
    assert_eq!(stream.next().await.unwrap(), "How are you?");
}

#[tokio::test]
async fn sync_and_async_outputs_match() {
    let text = "Alpha. Beta? Gamma!";
    let expected = segment_text_into_sentences(text);
    let chunks = vec![Ok::<_, ()>(text.to_string())];
    let mut stream = Box::pin(sentence_stream(tokio_stream::iter(chunks)));
    let mut from_stream = Vec::new();
    while let Some(chunk) = stream.next().await {
        from_stream.push(chunk.unwrap());
    }
    assert_eq!(expected, from_stream);
}
