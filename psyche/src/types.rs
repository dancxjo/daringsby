use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

pub use lingproc::ImageData;
pub type Decision = lingproc::Decision<crate::HostInstruction>;

/// Latitude/longitude coordinates from a positioning sensor.
#[cfg_attr(feature = "ts", derive(TS))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoLoc {
    /// Longitude in decimal degrees.
    pub longitude: f64,
    /// Latitude in decimal degrees.
    pub latitude: f64,
    /// RFC3339 time when this location was observed by the device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
}

pub fn parse_observed_at(at: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn image_captured_at(image: &ImageData) -> Option<DateTime<Utc>> {
    image.captured_at.as_deref().and_then(parse_observed_at)
}

pub fn geoloc_observed_at(loc: &GeoLoc) -> Option<DateTime<Utc>> {
    loc.observed_at.as_deref().and_then(parse_observed_at)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub label: Option<String>,
    pub embedding: Vec<f32>,
}

/// Timestamp emitted periodically by [`HeartbeatSensor`](crate::HeartbeatSensor).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Moment of the heartbeat.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
