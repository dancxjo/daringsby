use chrono::{DateTime, Utc};
use std::sync::{
    Mutex,
    mpsc::{Receiver, Sender, channel},
};

/// A single sensory input tagged with when it was perceived.
#[derive(Debug, Clone, PartialEq)]
pub struct Sensation<T> {
    /// Timestamp for when the sensation was recorded.
    pub when: DateTime<Utc>,
    /// Raw sensory value.
    pub what: T,
}

impl<T> Sensation<T> {
    /// Create a new `Sensation` happening right now.
    pub fn new(value: T) -> Self {
        Self {
            when: Utc::now(),
            what: value,
        }
    }
}

/// Collection of sensations plus a textual interpretation.
#[derive(Debug, Clone, PartialEq)]
pub struct Experience<T> {
    /// Sensory inputs composing this experience.
    pub what: Vec<Sensation<T>>,
    /// How the psyche interprets them.
    pub how: String,
}

impl<T> Experience<T> {
    /// Create an experience from sensations and descriptive text.
    pub fn new(what: Vec<Sensation<T>>, how: impl Into<String>) -> Self {
        Self {
            what,
            how: how.into(),
        }
    }
}

/// Reactive sensor producing sensations for subscribers.
pub trait Sensor<T>: Send + Sync {
    /// Provide a new input to the sensor.
    fn feel(&self, sensation: Sensation<T>);
    /// Subscribe to future emitted sensations.
    fn subscribe(&self) -> Receiver<Sensation<T>>;
}

/// Subject sensor broadcasting sensations to subscribers with optional filtering.
pub struct SubjectSensor<T> {
    subscribers: Mutex<Vec<Sender<Sensation<T>>>>,
    filter: Box<dyn Fn(&Sensation<T>) -> bool + Send + Sync>,
}

impl<T> SubjectSensor<T> {
    /// Create a new `SubjectSensor` with the provided filter.
    pub fn new<F>(filter: F) -> Self
    where
        F: Fn(&Sensation<T>) -> bool + Send + Sync + 'static,
    {
        Self {
            subscribers: Mutex::new(Vec::new()),
            filter: Box::new(filter),
        }
    }
}

impl<T: Clone + Send + 'static> Sensor<T> for SubjectSensor<T> {
    fn feel(&self, sensation: Sensation<T>) {
        if !(self.filter)(&sensation) {
            return;
        }
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| tx.send(sensation.clone()).is_ok());
    }

    fn subscribe(&self) -> Receiver<Sensation<T>> {
        let (tx, rx) = channel();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtered_sensations_reach_subscribers() {
        let sensor: SubjectSensor<u8> = SubjectSensor::new(|s| s.what % 2 == 0);
        let rx = sensor.subscribe();
        sensor.feel(Sensation::new(1));
        sensor.feel(Sensation::new(2));
        assert_eq!(rx.recv().unwrap().what, 2);
    }

    #[test]
    fn multiple_subscribers_receive() {
        let sensor: SubjectSensor<&str> = SubjectSensor::new(|_| true);
        let rx1 = sensor.subscribe();
        let rx2 = sensor.subscribe();
        sensor.feel(Sensation::new("hi"));
        assert_eq!(rx1.recv().unwrap().what, "hi");
        assert_eq!(rx2.recv().unwrap().what, "hi");
    }
}
