use async_trait::async_trait;

/// Generic sensor trait for delivering typed sensations to the psyche.
#[async_trait]
pub trait Sensor<T>: Send + Sync {
    /// Forward a sensed input of type `T`.
    async fn sense(&self, input: T);
}
