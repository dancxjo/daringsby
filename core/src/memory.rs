use crate::types::Impression;

/// Simple sink for storing impressions.
pub trait Memory: Send + Sync {
    /// Archive the given impression.
    fn remember(&mut self, imp: &Impression);
}

/// Placeholder implementation that discards impressions.
pub struct NoopMemory;

impl Memory for NoopMemory {
    fn remember(&mut self, _imp: &Impression) {}
}
