use pete::dummy_psyche;
use psyche::{ImageData, Sensation};
use tokio::time::Duration;

#[tokio::test]
#[ignore]
async fn vision_wit_receives_images() {
    let psyche = dummy_psyche();
    psyche::enable_debug("Vision").await;
    let mut reports = psyche.wit_reports();
    let tx = psyche.input_sender();
    let handle = tokio::spawn(async move { psyche.run().await });

    tx.send(Sensation::Of(Box::new(ImageData {
        mime: "image/png".into(),
        base64: "zzz".into(),
    })))
    .await
    .unwrap();

    let mut got = false;
    for _ in 0..5 {
        if let Ok(Ok(r)) = tokio::time::timeout(Duration::from_millis(50), reports.recv()).await {
            if r.name == "Vision" {
                got = true;
                break;
            }
        }
    }

    handle.abort();
    let _ = handle.await;
    psyche::disable_debug("Vision").await;
    assert!(got);
}
