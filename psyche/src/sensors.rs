use crate::{Experience, Sensation, Sensor, bus::Event};

/// Sensor interpreting chat events from the bus.
///
/// # Examples
/// ```
/// use psyche::{bus::Event, sensors::ChatSensor, Sensation, Sensor};
/// let mut sensor = ChatSensor::default();
/// let exp = sensor.feel(Sensation::new(Event::Chat("hi".into()))).unwrap();
/// assert_eq!(exp.how, "I heard someone say: hi");
/// ```
#[derive(Default)]
pub struct ChatSensor;

impl Sensor for ChatSensor {
    type Input = Event;
    fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
        match s.what {
            Event::Chat(line) => Some(Experience::new(format!("I heard someone say: {line}"))),
            _ => None,
        }
    }
}

/// Sensor interpreting connection events from the bus.
///
/// # Examples
/// ```
/// use std::net::SocketAddr;
/// use psyche::{bus::Event, sensors::ConnectionSensor, Sensation, Sensor};
/// let mut sensor = ConnectionSensor::default();
/// let addr: SocketAddr = "127.0.0.1:80".parse().unwrap();
/// let exp = sensor.feel(Sensation::new(Event::Connected(addr))).unwrap();
/// assert!(exp.how.contains("127.0.0.1"));
/// ```
#[derive(Default)]
pub struct ConnectionSensor;

impl Sensor for ConnectionSensor {
    type Input = Event;
    fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
        match s.what {
            Event::Connected(addr) => {
                Some(Experience::new(format!("Someone at {addr} connected.")))
            }
            Event::Disconnected(addr) => {
                Some(Experience::new(format!("Connection from {addr} closed.")))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_event_to_experience() {
        let mut sensor = ChatSensor::default();
        let exp = sensor
            .feel(Sensation::new(Event::Chat("hello".into())))
            .unwrap();
        assert_eq!(exp.how, "I heard someone say: hello");
    }

    #[test]
    fn connection_events_to_experience() {
        let addr: std::net::SocketAddr = "127.0.0.1:80".parse().unwrap();
        let mut sensor = ConnectionSensor::default();
        let exp = sensor.feel(Sensation::new(Event::Connected(addr))).unwrap();
        assert_eq!(exp.how, "Someone at 127.0.0.1:80 connected.");
        let exp = sensor
            .feel(Sensation::new(Event::Disconnected(addr)))
            .unwrap();
        assert_eq!(exp.how, "Connection from 127.0.0.1:80 closed.");
    }
}
