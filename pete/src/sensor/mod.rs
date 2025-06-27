#[cfg(feature = "eye")]
pub mod eye;
#[cfg(feature = "geo")]
pub mod geo;
pub mod heartbeat;
pub mod self_discovery;

use async_trait::async_trait;
use psyche::traits::Sensor;

/// Placeholder sensor used when a feature is disabled.
#[derive(Clone)]
pub struct NoopSensor;

#[async_trait]
impl<T: Send + 'static> Sensor<T> for NoopSensor {
    async fn sense(&self, _input: T) {}
    fn describe(&self) -> &'static str {
        "disabled"
    }
}
