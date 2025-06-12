use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

use crate::{Psyche, Scheduler};

/// Spawn a background task that polls external sensors and drives the heart.
///
/// The returned handle can be awaited or aborted when shutting down.
///
/// # Examples
/// ```ignore
/// use pete::sensors::HeartbeatSensor;
/// use psyche::{spawn_heartbeat, JoinScheduler, Psyche};
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
///
/// let psyche = Arc::new(Mutex::new(Psyche::new(|| JoinScheduler::default(), vec![
///     Box::new(HeartbeatSensor::new(std::time::Duration::from_secs(0)))
/// ])));
/// let handle = spawn_heartbeat(psyche.clone());
/// handle.abort();
/// ```
pub fn spawn_heartbeat<S>(psyche: Arc<TokioMutex<Psyche<S>>>) -> tokio::task::JoinHandle<()>
where
    S: Scheduler + Send + 'static,
    S::Output: Clone + Into<String> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let sleep = {
                let mut p = psyche.lock().await;
                p.poll_sensors();
                p.heart.beat();
                std::time::Duration::from_millis(p.heart.due_ms())
            };
            tokio::time::sleep(sleep).await;
        }
    })
}

/// Start a blocking thread driving the heart in a loop.
pub fn start_heartbeat_thread<S>(psyche: Arc<Mutex<Psyche<S>>>) -> std::thread::JoinHandle<()>
where
    S: Scheduler + Send + 'static,
    S::Output: Clone + Into<String> + Send + 'static,
{
    std::thread::spawn(move || {
        loop {
            let sleep = {
                let mut p = psyche.lock().unwrap();
                p.poll_sensors();
                p.heart.beat();
                std::time::Duration::from_millis(p.heart.due_ms())
            };
            std::thread::sleep(sleep);
        }
    })
}
