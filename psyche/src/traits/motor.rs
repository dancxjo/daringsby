use async_trait::async_trait;

/// Host-side actions Pete can take.
///
/// Only the `Will` should invoke these. These typed behaviors are distinct from
/// [`crate::motorcall::InstructionExecutor`], which handles generic instruction
/// tags parsed from language model output.
#[async_trait]
pub trait Motor: Send + Sync {
    /// Speak `text` using the configured mouth.
    async fn say(&self, text: &str);
    /// Update Pete's emotional expression to `emoji`.
    async fn set_emotion(&self, emoji: &str);
    /// Capture a photo from the active camera.
    async fn take_photo(&self);
    /// Focus on the given `name`, e.g. a person.
    async fn focus_on(&self, name: &str);
}

/// [`Motor`] implementation that does nothing.
#[derive(Clone, Default)]
pub struct NoopMotor;

#[async_trait]
impl Motor for NoopMotor {
    async fn say(&self, _text: &str) {}
    async fn set_emotion(&self, _emoji: &str) {}
    async fn take_photo(&self) {}
    async fn focus_on(&self, _name: &str) {}
}

#[cfg(doctest)]
mod docs {
    /// ```no_run
    /// use psyche::traits::{Motor, NoopMotor};
    /// # async fn run() {
    /// let motor = NoopMotor;
    /// motor.say("Hello").await;
    /// # }
    /// ```
    fn _doctest() {}
}
