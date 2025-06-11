use crate::Sensation;

/// A collection of timestamped memories.
///
/// ```
/// use psyche::{Memory, Sensation};
/// let mut m = Memory::new();
/// m.remember(Sensation::new(42));
/// assert_eq!(m.all().len(), 1);
/// ```
#[derive(Default)]
pub struct Memory<T> {
    entries: Vec<Sensation<T>>,
}

impl<T> Memory<T> {
    /// Create an empty memory.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Store a sensation for later recall.
    pub fn remember(&mut self, s: Sensation<T>) {
        log::info!("remembering sensation");
        self.entries.push(s);
    }

    /// Access all recorded sensations.
    pub fn all(&self) -> &[Sensation<T>] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_records() {
        let mut mem = Memory::new();
        mem.remember(Sensation::new(1u8));
        assert_eq!(mem.all().len(), 1);
    }
}
