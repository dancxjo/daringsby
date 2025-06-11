use crate::{Experience, Sensation};

/// Convert a batch of experiences into a new sensation.
pub trait Scheduler {
    type Output;
    fn schedule(&mut self, prompt: &str, batch: Vec<Experience>)
    -> Option<Sensation<Self::Output>>;
}
