use psyche::{Event, WitReport};
use tokio::sync::{broadcast, mpsc};

/// Central communication hub for Pete events and logs.
#[derive(Clone)]
pub struct EventBus {
    events: broadcast::Sender<Event>,
    logs: broadcast::Sender<String>,
    wits: broadcast::Sender<WitReport>,
    input: mpsc::UnboundedSender<String>,
}

impl EventBus {
    /// Create a new `EventBus` wrapping existing channels.
    ///
    /// Returns the bus and a receiver for user input.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
        let (events, _) = broadcast::channel(16);
        let (logs, _) = broadcast::channel(100);
        let (wits, _) = broadcast::channel(16);
        let (input, rx) = mpsc::unbounded_channel();
        (
            Self {
                events,
                logs,
                wits,
                input,
            },
            rx,
        )
    }

    /// Send an [`Event`] to all subscribers.
    pub fn publish_event(&self, event: Event) {
        let _ = self.events.send(event);
    }

    /// Subscribe to [`Event`]s published on the bus.
    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    /// Obtain the event sender for direct use.
    pub fn event_sender(&self) -> broadcast::Sender<Event> {
        self.events.clone()
    }

    /// Send a log line to listeners.
    pub fn log(&self, msg: impl Into<String>) {
        let _ = self.logs.send(msg.into());
    }

    /// Subscribe to log messages.
    pub fn subscribe_logs(&self) -> broadcast::Receiver<String> {
        self.logs.subscribe()
    }

    /// Publish a [`WitReport`].
    pub fn publish_wit(&self, report: WitReport) {
        let _ = self.wits.send(report);
    }

    /// Subscribe to [`WitReport`]s.
    pub fn subscribe_wits(&self) -> broadcast::Receiver<WitReport> {
        self.wits.subscribe()
    }

    /// Obtain a sender for incoming user text.
    pub fn user_input_sender(&self) -> mpsc::UnboundedSender<String> {
        self.input.clone()
    }

    /// Access the log sender for initialization.
    pub fn log_sender(&self) -> broadcast::Sender<String> {
        self.logs.clone()
    }
}
