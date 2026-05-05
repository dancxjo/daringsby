use pete::{ChannelEar, dummy_psyche, listen_user_input};
use psyche::{Sensation, traits::Ear};
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;

#[tokio::test]
async fn records_user_input() {
    let psyche = dummy_psyche();
    let conv = psyche.conversation();
    let voice = psyche.voice();
    let speaking = std::sync::Arc::new(AtomicBool::new(false));
    let ear = std::sync::Arc::new(ChannelEar::new(
        psyche.input_sender(),
        speaking,
        voice.clone(),
    ));
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(listen_user_input(rx, ear, voice));
    let handle = tokio::spawn(async move { psyche.run().await });

    tx.send("hello".to_string()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    handle.abort();
    let _ = handle.await;

    let log_len = { conv.lock().await.all().len() };
    assert_eq!(log_len, 1);
}

#[tokio::test]
async fn channel_ear_does_not_block_when_psyche_input_is_full() {
    let psyche = dummy_psyche();
    let voice = psyche.voice();
    let speaking = std::sync::Arc::new(AtomicBool::new(false));
    let (tx, _rx) = mpsc::channel(1);
    tx.send(Sensation::heard_user_voice("queued"))
        .await
        .unwrap();
    let ear = ChannelEar::new(tx, speaking, voice);

    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        ear.hear_user_say("still listening"),
    )
    .await
    .expect("ear blocked on a full psyche input queue");
}
