use once_cell::sync::OnceCell;
use tokio::sync::broadcast;

/// Events emitted by the system.
#[derive(Clone, Debug)]
pub enum Event {
    /// Log line created via [`log`].
    Log(String),
    /// Chat line submitted from a user.
    Chat(String),
    /// WebSocket client connected from an address.
    Connected(std::net::SocketAddr),
    /// WebSocket client disconnected.
    Disconnected(std::net::SocketAddr),
}

/// Simple broadcast bus for sending [`Event`]s to multiple listeners.
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    /// Obtain a receiver subscribed to all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Broadcast an event to all subscribers. Errors are ignored.
    pub fn send(&self, evt: Event) {
        let _ = self.sender.send(evt);
    }
}

static BUS: OnceCell<EventBus> = OnceCell::new();

/// Access the global event bus used for logging.
pub fn global_bus() -> &'static EventBus {
    BUS.get_or_init(EventBus::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_and_receive_chat() {
        let bus = global_bus();
        let mut rx = bus.subscribe();
        bus.send(Event::Chat("hi".into()));
        match rx.recv().await {
            Ok(Event::Chat(line)) => assert_eq!(line, "hi"),
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn send_and_receive_connection() {
        use std::net::SocketAddr;
        let bus = global_bus();
        let mut rx = bus.subscribe();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        bus.send(Event::Connected(addr));
        match rx.recv().await {
            Ok(Event::Connected(a)) => assert_eq!(a, addr),
            other => panic!("unexpected event: {:?}", other),
        }
    }
}
