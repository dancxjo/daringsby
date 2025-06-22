use async_trait::async_trait;
use std::any::Any;

/// Observer of raw [`Sensation`] inputs.
///
/// Implementations may process incoming sensations from sensors or
/// the conversation loop.
#[async_trait]
pub trait SensationObserver: Send + Sync {
    /// Handle an incoming payload from the psyche.
    async fn observe_sensation(&self, payload: &(dyn Any + Send + Sync));
}
