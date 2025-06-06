use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sensation {
    /// Timestamp of when the sensation was perceived
    pub when: DateTime<Utc>,
    /// Brief interpretation of how Pete feels about it
    pub how: String,
    /// Optional raw content of what was perceived
    pub what: Option<String>,
}

impl Sensation {
    /// Create a new sensation with the current timestamp
    pub fn new(how: impl Into<String>, what: Option<impl Into<String>>) -> Self {
        Self {
            when: Utc::now(),
            how: how.into(),
            what: what.map(|w| w.into()),
        }
    }
}
