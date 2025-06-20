use crate::Sensation;
use async_trait::async_trait;

/// Observer of raw [`Sensation`] inputs.
///
/// Implementations may process incoming sensations from sensors or
/// the conversation loop.
#[async_trait]
pub trait SensationObserver: Send + Sync {
    /// Handle an incoming [`Sensation`].
    async fn observe_sensation(&self, sensation: &Sensation);
}
