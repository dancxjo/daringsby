use pete::SelfDiscoverySensor;
use psyche::{Impression, Sensation};
use tokio::sync::mpsc;

#[tokio::test(start_paused = true)]
async fn emits_first_sentence() {
    let (tx, mut rx) = mpsc::channel(1);
    let _sensor = SelfDiscoverySensor::test_interval(tx, 1);
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    let s = rx.recv().await.expect("impression");
    if let Sensation::Of(any) = s {
        let imp = any.downcast_ref::<Impression<()>>().unwrap();
        assert!(imp.summary.starts_with("You are the narrator"));
    } else {
        panic!("unexpected sensation");
    }
}
