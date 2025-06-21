use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageData {
    pub mime: String,
    pub base64: String,
}

/// Latitude/longitude coordinates from a positioning sensor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoLoc {
    /// Longitude in decimal degrees.
    pub longitude: f64,
    /// Latitude in decimal degrees.
    pub latitude: f64,
}
