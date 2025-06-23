use futures::StreamExt;
use lingproc::{segment_text_into_sentences, sentence_stream};
use tokio_stream::iter;

#[tokio::test]
async fn sync_and_stream_outputs_match() {
    let text = "Hello world. How are you?";
    let expected = segment_text_into_sentences(text);
    let chunks = vec![Ok::<String, ()>(text.to_string())];
    let actual: Vec<String> = sentence_stream(iter(chunks))
        .map(|r| r.unwrap().trim().to_string())
        .collect()
        .await;
    assert_eq!(expected, actual);
}

#[tokio::test]
async fn segment_text_into_sentences_splits() {
    let sentences = segment_text_into_sentences("Hello world. How are you?");
    assert_eq!(
        sentences,
        vec!["Hello world.".to_string(), "How are you?".to_string()]
    );
}
