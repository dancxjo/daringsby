use async_trait::async_trait;
use psyche::{GeoLoc, Sensation, Sensor};
use tokio::sync::mpsc;
use tracing::debug;

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
        debug!("geo sensor received location");
        let _ = self.forward.send(Sensation::Of(Box::new(loc)));
    }

    fn description(&self) -> String {
        "GPS: Streams geolocation coordinates.".to_string()
    }
}
