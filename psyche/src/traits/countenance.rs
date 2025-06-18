/// Emotional display output.
///
/// Implementations update some representation of Pete's face or
/// an external UI element with an emoji.
pub trait Countenance: Send + Sync {
    /// Express the provided emoji, e.g. "😊".
    fn express(&self, emoji: &str);
}

/// [`Countenance`] implementation that does nothing.
#[derive(Clone, Default)]
pub struct NoopCountenance;

impl Countenance for NoopCountenance {
    fn express(&self, _emoji: &str) {}
}
