use async_trait::async_trait;
use psyche::{GeoLoc, Sensation, Sensor};
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Sensor forwarding geolocation updates to the psyche.
#[derive(Clone)]
pub struct GeoSensor {
    forward: mpsc::UnboundedSender<Sensation>,
}

impl GeoSensor {
    /// Create a new `GeoSensor` using the provided channel.
    pub fn new(forward: mpsc::UnboundedSender<Sensation>) -> Self {
        Self { forward }
    }
}

#[async_trait]
impl Sensor<GeoLoc> for GeoSensor {
    async fn sense(&self, loc: GeoLoc) {
        info!("geo sensor received location");
        debug!("geo sensor received location");
        let _ = self.forward.send(Sensation::Of(Box::new(loc)));
    }

    fn describe(&self) -> &'static str {
        "Pete knows where he is in terms of latitude and longitude. This may \
help him remember where events happened."
    }
}
