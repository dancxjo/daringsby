use serde::Serialize;
use std::time::SystemTime;

/// Input data captured by the system.
///
/// `Sensation` wraps any data with a timestamp so the moment of
/// perception is remembered.
///
/// # Examples
/// ```
/// use psyche::Sensation;
/// let s = Sensation::new(42);
/// assert_eq!(s.what, 42);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sensation<T> {
    /// Time when the data was perceived.
    pub when: SystemTime,
    /// Arbitrary data representing the perception.
    pub what: T,
}

impl<T> Sensation<T> {
    /// Create a new sensation stamped with `SystemTime::now()`.
    pub fn new(data: T) -> Self {
        Self {
            when: SystemTime::now(),
            what: data,
        }
    }

    /// Create a sensation with a specified timestamp.
    pub fn with_timestamp(data: T, timestamp: SystemTime) -> Self {
        Self {
            when: timestamp,
            what: data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_sensation() {
        let s = Sensation::new(123u8);
        assert_eq!(s.what, 123);
    }
}
