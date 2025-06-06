use sensor::{heartbeat::HeartbeatSensor, Sensor};
use tokio::sync::mpsc;
use std::time::Duration;

#[tokio::test]
async fn heartbeat_emits() {
    let (tx, mut rx) = mpsc::channel(2);
    let mut hb = HeartbeatSensor::new(Duration::from_millis(100));
    tokio::spawn(async move { hb.run(tx).await; });
    let s = rx.recv().await.unwrap();
    assert!(s.how.contains("Heartbeat"));
}
