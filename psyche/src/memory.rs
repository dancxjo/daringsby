use crate::Event;
use tokio::sync::broadcast;
use tracing::info;

/// Simple memory that emits Cypher merge statements for text inputs.
///
/// This struct is a placeholder representing a memory subsystem that
/// would typically persist facts in a graph database like Neo4j. It
/// collects text observations via [`feel`] and when [`consult`] is
/// called it emits a Cypher statement on the provided broadcast
/// channel.
pub struct Memory {
    tx: broadcast::Sender<Event>,
    log: Vec<String>,
}

impl Memory {
    /// Create a new `Memory` using the given event channel.
    pub fn new(tx: broadcast::Sender<Event>) -> Self {
        Self {
            tx,
            log: Vec::new(),
        }
    }

    /// Record a new observation.
    pub fn feel(&mut self, text: impl Into<String>) {
        self.log.push(text.into());
    }

    /// Emit a Cypher statement summarizing the last observation.
    pub async fn consult(&mut self) -> anyhow::Result<()> {
        if let Some(last) = self.log.last() {
            let cypher = format!("MERGE (:Memory {{ text: \"{}\" }})", last);
            info!(%cypher, "memory generated cypher");
            let _ = self.tx.send(Event::StreamChunk(cypher));
        }
        Ok(())
    }
}
