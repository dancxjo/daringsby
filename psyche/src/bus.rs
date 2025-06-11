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
    /// A processor started handling a prompt.
    ProcessorPrompt { name: String, prompt: String },
    /// A processor produced a chunk of output.
    ProcessorChunk { name: String, chunk: String },
}

/// Simple broadcast bus for sending [`Event`]s to multiple listeners.
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    /// Create a new event bus.
    pub fn new() -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_and_receive_chat() {
        let bus = EventBus::new();
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
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        bus.send(Event::Connected(addr));
        match rx.recv().await {
            Ok(Event::Connected(a)) => assert_eq!(a, addr),
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn send_and_receive_processor_events() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.send(Event::ProcessorPrompt {
            name: "p".into(),
            prompt: "hi".into(),
        });
        match rx.recv().await {
            Ok(Event::ProcessorPrompt { name, prompt }) => {
                assert_eq!(name, "p");
                assert_eq!(prompt, "hi");
            }
            other => panic!("unexpected event: {:?}", other),
        }
        bus.send(Event::ProcessorChunk {
            name: "p".into(),
            chunk: "c".into(),
        });
        match rx.recv().await {
            Ok(Event::ProcessorChunk { name, chunk }) => {
                assert_eq!(name, "p");
                assert_eq!(chunk, "c");
            }
            other => panic!("unexpected event: {:?}", other),
        }
    }
}
