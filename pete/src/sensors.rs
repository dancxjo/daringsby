use psyche::{Experience, Sensation, Sensor, bus::Event};

/// Sensor interpreting chat events from the bus.
///
/// # Examples
/// ```
/// use psyche::{bus::Event, Sensation, Sensor};
/// use pete::sensors::ChatSensor;
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
/// use psyche::{bus::Event, Sensation, Sensor};
/// use pete::sensors::ConnectionSensor;
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
            _ => Vec::new(),
        }
    }
}

/// Periodic sensor announcing the local time and that Pete is alive.
///
/// # Examples
/// ```
/// use std::time::Duration;
/// use psyche::Sensor;
/// use pete::sensors::HeartbeatSensor;
/// let mut sensor = HeartbeatSensor::new(Duration::from_secs(0));
/// let exps = sensor.experience();
/// assert!(exps[0].how.contains("still beating"));
/// ```
pub struct HeartbeatSensor {
    interval: std::time::Duration,
    next: Option<std::time::Instant>,
}

impl HeartbeatSensor {
    /// Create a sensor that emits every `interval` seconds.
    pub fn new(interval: std::time::Duration) -> Self {
        Self {
            interval,
            next: None,
        }
    }
}

impl Default for HeartbeatSensor {
    fn default() -> Self {
        Self::new(std::time::Duration::from_secs(30))
    }
}

impl Sensor for HeartbeatSensor {
    type Input = Event;
    fn feel(&mut self, _s: Sensation<Self::Input>) {}

    fn experience(&mut self) -> Vec<Experience> {
        let now = std::time::Instant::now();
        if self.next.map_or(true, |next| now >= next) {
            self.next = Some(now + self.interval);
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            vec![Experience::new(format!(
                "It's {ts} and Pete's heart is still beating."
            ))]
        } else {
            Vec::new()
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

    #[test]
    fn no_connection_event_yields_empty() {
        let mut sensor = ConnectionSensor::default();
        assert!(sensor.experience().is_empty());
    }

    #[test]
    fn heartbeat_emits_message() {
        let mut sensor = HeartbeatSensor::new(std::time::Duration::from_secs(1));
        let exps = sensor.experience();
        assert!(exps[0].how.contains("still beating"));
        assert!(sensor.experience().is_empty());
    }
}
