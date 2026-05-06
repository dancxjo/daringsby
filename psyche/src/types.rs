use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

/// Three-axis browser motion vector, usually reported by device sensors.
#[cfg_attr(feature = "ts", derive(TS))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotionVector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
}

/// Browser device orientation in degrees.
#[cfg_attr(feature = "ts", derive(TS))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceOrientation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gamma: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute: Option<bool>,
}

/// Motion data reported by browser DeviceMotion/DeviceOrientation APIs.
#[cfg_attr(feature = "ts", derive(TS))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserMotion {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceleration: Option<MotionVector>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceleration_including_gravity: Option<MotionVector>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_rate: Option<DeviceOrientation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<DeviceOrientation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<f64>,
    /// RFC3339 time when this motion reading was observed by the device.
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

pub fn image_content_id(image: &ImageData) -> String {
    stable_content_id("image", [&image.mime, &image.base64])
}

pub fn geoloc_observed_at(loc: &GeoLoc) -> Option<DateTime<Utc>> {
    loc.observed_at.as_deref().and_then(parse_observed_at)
}

pub fn browser_motion_observed_at(motion: &BrowserMotion) -> Option<DateTime<Utc>> {
    motion.observed_at.as_deref().and_then(parse_observed_at)
}

pub fn geoloc_content_id(loc: &GeoLoc) -> String {
    let longitude = format!("{:.7}", loc.longitude);
    let latitude = format!("{:.7}", loc.latitude);
    let observed_at = loc.observed_at.clone().unwrap_or_default();
    stable_content_id("geolocation", [&longitude, &latitude, &observed_at])
}

pub fn browser_motion_content_id(motion: &BrowserMotion) -> String {
    let json = serde_json::to_string(motion).unwrap_or_default();
    stable_content_id("browser-motion", [&json])
}

/// Unit-sphere vector for geospatial similarity.
pub fn geoloc_vector(loc: &GeoLoc) -> Vec<f32> {
    let lat = loc.latitude.to_radians();
    let lon = loc.longitude.to_radians();
    vec![
        (lat.cos() * lon.cos()) as f32,
        (lat.cos() * lon.sin()) as f32,
        lat.sin() as f32,
    ]
}

pub fn audio_captured_at(audio: &AudioClip) -> Option<DateTime<Utc>> {
    audio.captured_at.as_deref().and_then(parse_observed_at)
}

pub fn audio_clip_id(audio: &AudioClip) -> String {
    let sample_rate = audio.sample_rate.to_string();
    let channels = audio.channels.to_string();
    stable_content_id(
        "audio",
        [&audio.mime, &audio.base64, &sample_rate, &channels],
    )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub label: Option<String>,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioClip {
    pub mime: String,
    pub base64: String,
    pub sample_rate: u32,
    pub channels: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombobulationSummary {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_sensation_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub clip: AudioClip,
    pub clip_id: String,
    pub embedding: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageEmbedding {
    pub image: ImageData,
    pub image_id: String,
    pub embedding: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoEmbedding {
    pub loc: GeoLoc,
    pub geoloc_id: String,
    pub embedding: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

fn stable_content_id<'a>(prefix: &str, parts: impl IntoIterator<Item = &'a String>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("{prefix}:sha256:{:x}", hasher.finalize())
}

/// Timestamp emitted periodically by [`HeartbeatSensor`](crate::HeartbeatSensor).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Moment of the heartbeat.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
