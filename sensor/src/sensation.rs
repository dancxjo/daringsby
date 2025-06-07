use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SensationData {
    Text(String),
    Image(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sensation {
    /// Timestamp of when the sensation was perceived
    pub when: DateTime<Utc>,
    /// Brief interpretation of how Pete feels about it
    pub how: String,
    /// Optional raw content of what was perceived
    pub data: Option<SensationData>,
}

impl Sensation {
    /// Create a new sensation with the current timestamp
    pub fn new(how: impl Into<String>, what: Option<impl Into<String>>) -> Self {
        Self {
            when: Utc::now(),
            how: how.into(),
            data: what.map(|w| SensationData::Text(w.into())),
        }
    }

    /// Construct a vision sensation from raw image bytes.
    pub fn saw(image: Vec<u8>) -> Self {
        Self {
            when: Utc::now(),
            how: "eye".into(),
            data: Some(SensationData::Image(image)),
        }
    }
}
