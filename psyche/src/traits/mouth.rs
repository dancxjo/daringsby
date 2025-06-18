use async_trait::async_trait;
///
/// A `Mouth` is responsible for turning text into audio or visual output.
/// Implementations must be `Send` and `Sync` so they can be shared across tasks.
/// Calls to its methods are made sequentially by [`Psyche`].
#[async_trait]
pub trait Mouth: Send + Sync {
    /// Asynchronously vocalize `text`.
    ///
    /// The returned future resolves once the speech is completed.
    async fn speak(&self, text: &str);
    /// Interrupt any in-progress speech.
    async fn interrupt(&self);
    /// Whether the mouth is currently speaking.
    /// Return `true` if speech is currently being produced.
    fn speaking(&self) -> bool;
}
