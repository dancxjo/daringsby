#[cfg(feature = "eye")]
pub mod eye;
#[cfg(feature = "geo")]
pub mod geo;

use async_trait::async_trait;
use psyche::Sensor;

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
