use pete::{dummy_psyche, listen_user_input};
use tokio::sync::mpsc;

#[tokio::test]
async fn records_user_input() {
    let mut psyche = dummy_psyche();
    let input = psyche.input_sender();
    let conv = psyche.conversation();
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(listen_user_input(rx, input, conv.clone()));

    tx.send("hello".to_string()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let log_len = { conv.lock().await.all().len() };
    assert_eq!(log_len, 1);
}
