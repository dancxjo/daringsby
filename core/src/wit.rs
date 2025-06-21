use crate::types::{Impression, Stimulus};
use async_trait::async_trait;

/// A cognitive process observing stimuli and optionally emitting an impression.
#[async_trait]
pub trait Wit: Send + Sync {
    /// Called on each tick with the current stimuli buffer.
    async fn tick(&mut self, inputs: Vec<Stimulus>) -> Option<Impression>;
    /// Human readable name for debugging.
    fn name(&self) -> &'static str;
}
