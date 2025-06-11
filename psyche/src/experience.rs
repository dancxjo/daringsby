use serde::Serialize;

/// Linguistic interpretation of a sensation.
///
/// `Experience` describes how a [`Sensation`](crate::Sensation) feels.
///
/// # Examples
/// ```
/// use psyche::Experience;
/// let e = Experience::new("I see a cat.");
/// assert_eq!(e.how, "I see a cat.");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Experience {
    /// Time when the sensation was interpreted.
    pub when: std::time::SystemTime,
    /// Text describing how the sensation feels.
    pub how: String,
}

impl Experience {
    /// Create a new experience from a descriptive phrase.
    pub fn new(how: impl Into<String>) -> Self {
        Self {
            when: std::time::SystemTime::now(),
            how: how.into(),
        }
    }

    /// Create a new experience with a specific timestamp.
    pub fn with_timestamp(how: impl Into<String>, when: std::time::SystemTime) -> Self {
        Self {
            when,
            how: how.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_experience() {
        let e = Experience::new("just a test");
        assert_eq!(e.how, "just a test");
    }
}
