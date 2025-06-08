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
#[derive(Debug, Clone, PartialEq)]
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

/// Linguistic interpretation of a sensation.
///
/// `Experience` is meant to be one sentence describing the input.
///
/// # Examples
/// ```
/// use psyche::Experience;
/// let e = Experience::new("I see a cat.");
/// assert_eq!(e.sentence, "I see a cat.");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Experience {
    /// The explanatory sentence.
    pub sentence: String,
}

/// Something that can transform a [`Sensation`] into an [`Experience`].
///
/// # Examples
/// ```
/// use psyche::{Experience, Sensation, Sensor};
/// struct Echo;
/// impl Sensor for Echo {
///     type Input = String;
///     fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
///         Some(Experience::new(s.what))
///     }
/// }
/// let mut sensor = Echo;
/// let exp = sensor.feel(Sensation::new("hello".to_string())).unwrap();
/// assert_eq!(exp.sentence, "hello");
/// ```
pub trait Sensor {
    /// Type of data this sensor accepts.
    type Input;

    /// Convert a sensation into an experience, if possible.
    fn feel(&mut self, sensation: Sensation<Self::Input>) -> Option<Experience>;
}

impl Experience {
    /// Create a new experience from a sentence.
    pub fn new(sentence: impl Into<String>) -> Self {
        Self {
            sentence: sentence.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Sensor for Echo {
        type Input = String;
        fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
            Some(Experience::new(s.what))
        }
    }

    #[test]
    fn create_sensation() {
        let s = Sensation::new(123u8);
        assert_eq!(s.what, 123);
    }

    #[test]
    fn create_experience() {
        let e = Experience::new("just a test");
        assert_eq!(e.sentence, "just a test");
    }

    #[test]
    fn echo_sensor() {
        let mut sensor = Echo;
        let exp = sensor.feel(Sensation::new("hi".to_string())).unwrap();
        assert_eq!(exp.sentence, "hi");
    }
}
