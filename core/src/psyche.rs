use crate::{memory::Memory, types::Stimulus, wit::Wit};
use serde_json;
use tracing::info;

/// Core processing loop combining multiple wits.
pub struct Psyche {
    /// Registered cognitive modules.
    pub wits: Vec<Box<dyn Wit>>,
    /// Recent stimuli available for reflection.
    pub stimuli: Vec<Stimulus>,
    /// Optional memory sink.
    pub memory: Option<Box<dyn Memory>>,
}

impl Psyche {
    /// Process one tick by invoking each wit.
    pub async fn tick(&mut self) {
        let mut new_impressions = vec![];

        for wit in self.wits.iter_mut() {
            if let Some(imp) = wit.tick(self.stimuli.clone()).await {
                info!(wit = wit.name(), summary = %imp.summary, emoji = ?imp.emoji, "wit fired");
                if let Some(mem) = self.memory.as_mut() {
                    mem.remember(&imp);
                }
                new_impressions.push(imp);
            }
        }

        for imp in &new_impressions {
            self.stimuli.push(Stimulus {
                what: serde_json::to_value(imp).expect("serialize"),
                timestamp: imp.timestamp,
            });
        }
    }
}
