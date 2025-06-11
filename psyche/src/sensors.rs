use crate::{Experience, Sensation, Sensor, bus::Event};

/// Sensor interpreting chat events from the bus.
///
/// # Examples
/// ```
/// use psyche::{bus::Event, sensors::ChatSensor, Sensation, Sensor};
/// let mut sensor = ChatSensor::default();
/// sensor.feel(Sensation::new(Event::Chat("hi".into())));
/// let exps = sensor.experience();
/// assert_eq!(exps[0].how, "I heard someone say: hi");
/// ```
#[derive(Default)]
pub struct ChatSensor {
    last: Option<String>,
}

impl Sensor for ChatSensor {
    type Input = Event;
    fn feel(&mut self, s: Sensation<Self::Input>) {
        if let Event::Chat(line) = s.what {
            self.last = Some(line);
        }
    }

    fn experience(&mut self) -> Vec<Experience> {
        match self.last.take() {
            Some(line) => vec![Experience::new(format!("I heard someone say: {line}"))],
            None => vec![Experience::new("I heard nothing.")],
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
/// sensor.feel(Sensation::new(Event::Connected(addr)));
/// let exps = sensor.experience();
/// assert!(exps[0].how.contains("127.0.0.1"));
/// ```
#[derive(Default)]
pub struct ConnectionSensor {
    last: Option<Event>,
}

impl Sensor for ConnectionSensor {
    type Input = Event;
    fn feel(&mut self, s: Sensation<Self::Input>) {
        match s.what {
            Event::Connected(_) | Event::Disconnected(_) => self.last = Some(s.what),
            _ => {}
        }
    }

    fn experience(&mut self) -> Vec<Experience> {
        match self.last.take() {
            Some(Event::Connected(addr)) => {
                vec![Experience::new(format!("Someone at {addr} connected."))]
            }
            Some(Event::Disconnected(addr)) => {
                vec![Experience::new(format!("Connection from {addr} closed."))]
            }
            _ => vec![Experience::new("No connection events.")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_event_to_experience() {
        let mut sensor = ChatSensor::default();
        sensor.feel(Sensation::new(Event::Chat("hello".into())));
        let exps = sensor.experience();
        assert_eq!(exps[0].how, "I heard someone say: hello");
    }

    #[test]
    fn connection_events_to_experience() {
        let addr: std::net::SocketAddr = "127.0.0.1:80".parse().unwrap();
        let mut sensor = ConnectionSensor::default();
        sensor.feel(Sensation::new(Event::Connected(addr)));
        let exps = sensor.experience();
        assert_eq!(exps[0].how, "Someone at 127.0.0.1:80 connected.");
        sensor.feel(Sensation::new(Event::Disconnected(addr)));
        let exps = sensor.experience();
        assert_eq!(exps[0].how, "Connection from 127.0.0.1:80 closed.");
    }
}
