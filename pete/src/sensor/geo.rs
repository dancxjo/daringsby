use async_trait::async_trait;
use psyche::traits::Sensor;
use psyche::{GeoLoc, Sensation};
use tokio::sync::mpsc;
use tracing::{debug, info};

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
        let _ = self.forward.send(Sensation::Of(Box::new(loc))).await;
    }

    fn describe(&self) -> &'static str {
        "Pete knows where he is in terms of latitude and longitude. This may \
help him remember where events happened."
    }
}
