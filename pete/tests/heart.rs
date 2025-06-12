use pete::sensors::HeartbeatSensor;
use psyche::spawn_heartbeat;
use psyche::{Heart, JoinScheduler, Psyche, Wit, bus::Event};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn heart_beats_continuously() {
    let sensors: Vec<Box<dyn psyche::Sensor<Input = Event> + Send + Sync>> = vec![Box::new(
        HeartbeatSensor::new(std::time::Duration::from_millis(10)),
    )];
    let make = || {
        Wit::with_config(
            JoinScheduler::default(),
            None,
            "w",
        )
    };
    let heart = Heart::new(make(), make(), make());
    let psyche = Arc::new(Mutex::new(Psyche::with_heart(heart, sensors)));
    let handle = spawn_heartbeat(psyche.clone());
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    {
        let p = psyche.lock().await;
        assert!(p.heart.quick.memory.all().len() > 1);
    }
    handle.abort();
}
