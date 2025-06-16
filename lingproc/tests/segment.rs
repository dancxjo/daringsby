use lingproc::{sentence_stream, word_stream};
use tokio_stream::wrappers::UnboundedReceiverStream;

#[tokio::test]
async fn sentence_buffering() {
    use tokio::sync::mpsc;
    let (tx, rx) = mpsc::unbounded_channel::<Result<String, ()>>();
    let mut sentences = Box::pin(sentence_stream(UnboundedReceiverStream::new(rx)));

    tx.send(Ok(
        "On Sat., Jun. 21, 1972, Mr. E. J. Picklesberger said, \"Whoa, Nelly!\" ".to_string(),
    ))
    .unwrap();
    tx.send(Ok("He looked around. Then".to_string())).unwrap();
    let first = futures::StreamExt::next(&mut sentences)
        .await
        .unwrap()
        .unwrap();
    assert!(first.starts_with("On Sat."));
}

#[tokio::test]
async fn splits_words() {
    let chunks: Vec<Result<String, ()>> = vec![Ok("Hello".into()), Ok(" world".into())];
    let mut words = Box::pin(word_stream(tokio_stream::iter(chunks)));
    assert_eq!(
        futures::StreamExt::next(&mut words).await.unwrap().unwrap(),
        "Hello"
    );
    assert_eq!(
        futures::StreamExt::next(&mut words).await.unwrap().unwrap(),
        "world"
    );
}
