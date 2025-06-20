use pete::{ChannelEar, dummy_psyche, listen_user_input};
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;

#[tokio::test]
async fn records_user_input() {
    let mut psyche = dummy_psyche();
    let conv = psyche.conversation();
    let speaking = std::sync::Arc::new(AtomicBool::new(false));
    let ear = std::sync::Arc::new(ChannelEar::new(
        psyche.input_sender(),
        conv.clone(),
        speaking,
    ));
    let (tx, rx) = mpsc::unbounded_channel();

    let voice = psyche.voice();
    tokio::spawn(listen_user_input(rx, ear, voice));

    tx.send("hello".to_string()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let log_len = { conv.lock().await.all().len() };
    assert_eq!(log_len, 1);
}
