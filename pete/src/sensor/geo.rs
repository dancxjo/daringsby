use async_trait::async_trait;
use psyche::{GeoLoc, Sensation, Sensor};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Sensor forwarding geolocation updates to the psyche.
#[derive(Clone)]
pub struct GeoSensor {
    forward: mpsc::Sender<Sensation>,
}

impl GeoSensor {
    /// Create a new `GeoSensor` using the provided channel.
    pub fn new(forward: mpsc::Sender<Sensation>) -> Self {
        Self { forward }
    }
}

#[async_trait]
impl Sensor<GeoLoc> for GeoSensor {
    async fn sense(&self, loc: GeoLoc) {
        info!("geo sensor received location");
        debug!("geo sensor received location");
        match self.forward.try_send(Sensation::Of(Box::new(loc))) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("dropping geolocation update because psyche input is full");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!("dropping geolocation update because psyche input is closed");
            }
        }
    }

    fn describe(&self) -> &'static str {
        "You know where you are in terms of latitude and longitude. This may \
help you remember where events happened."
    }
}
