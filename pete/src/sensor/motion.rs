use async_trait::async_trait;
use chrono::Utc;
use psyche::{
    BrowserMotion, Sensation, Sensor, browser_motion_observed_at,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Sensor forwarding browser motion updates to the psyche.
#[derive(Clone)]
pub struct MotionSensor {
    forward: mpsc::Sender<Sensation>,
}

impl MotionSensor {
    /// Create a new `MotionSensor` using the provided channel.
    pub fn new(forward: mpsc::Sender<Sensation>) -> Self {
        Self { forward }
    }
}

#[async_trait]
impl Sensor<BrowserMotion> for MotionSensor {
    async fn sense(&self, mut motion: BrowserMotion) {
        info!("motion sensor received browser motion");
        debug!("motion sensor received browser motion");
        let occurred_at = browser_motion_observed_at(&motion).unwrap_or_else(Utc::now);
        if motion.observed_at.is_none() {
            motion.observed_at = Some(occurred_at.to_rfc3339());
        }
        match self
            .forward
            .try_send(Sensation::of_at(motion, occurred_at))
        {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("dropping browser motion update because psyche input is full");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!("dropping browser motion update because psyche input is closed");
            }
        }
    }

    fn describe(&self) -> &'static str {
        "You can feel browser device motion: acceleration, gravity, rotation, \
and orientation from the device sensors."
    }
}
