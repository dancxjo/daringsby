use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{Duration, sleep};

use crate::{Psyche, Scheduler};

/// Spawn a background task that polls sensors and advances the heart.
///
/// The task only triggers a beat when new experiences are waiting so that just
/// one linguistic task is dispatched per beat. When idle, the loop briefly
/// sleeps to avoid a tight spin.
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
            // log::info!("Tick");
            let mut p = psyche.lock().await;
            p.poll_sensors();
            p.heart.beat();
            drop(p);
        }
    })
}
