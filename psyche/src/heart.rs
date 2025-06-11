use crate::{Experience, Scheduler, Sensation, Sensor, Wit};

/// A single layer heart containing just the "quick" [`Wit`].
///
/// `Heart` implements [`Sensor`], forwarding sensations directly to the quick
/// wit. Call [`Heart::beat`] to process queued sensations and store the
/// resulting experiences.
pub struct Heart<W> {
    /// The sole wit handling all reflection.
    pub quick: W,
    /// Buffer of experiences returned from the last [`beat`](Self::beat) call.
    pub buffer: Vec<Experience>,
}

impl<W> Heart<W> {
    /// Create a heart from a single quick wit.
    pub fn new(quick: W) -> Self {
        Self {
            quick,
            buffer: Vec::new(),
        }
    }

    /// Reference to the quick wit.
    pub fn quick(&self) -> Option<&W> {
        Some(&self.quick)
    }

    /// Mutable reference to the quick wit.
    pub fn quick_mut(&mut self) -> Option<&mut W> {
        Some(&mut self.quick)
    }
}

impl<S> Heart<Wit<S>>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// Process queued sensations and store produced experiences.
    ///
    /// ```
    /// use psyche::{Heart, JoinScheduler, Wit, Sensation, Sensor, Experience};
    /// let mut heart = Heart::new(Wit::new(JoinScheduler::default(), "q"));
    /// heart.feel(Sensation::new(Experience::new("hi")));
    /// heart.beat();
    /// assert!(!heart.buffer.is_empty());
    /// ```
    pub fn beat(&mut self) {
        let outputs = self.quick.experience();
        self.buffer.extend(outputs);
    }
}

impl<S> Sensor for Heart<Wit<S>>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    type Input = Experience;

    fn feel(&mut self, sensation: Sensation<Self::Input>) {
        log::info!("heart feel: {}", sensation.what.how);
        self.quick.feel(sensation);
    }

    fn experience(&mut self) -> Vec<Experience> {
        self.beat();
        std::mem::take(&mut self.buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JoinScheduler, Sensation, Sensor, Wit};

    #[test]
    fn beat_stores_output() {
        let mut heart = Heart::new(Wit::with_config(
            JoinScheduler::default(),
            Some("quick".into()),
            std::time::Duration::from_secs(0),
            "quick",
        ));
        heart.feel(Sensation::new(Experience::new("hello")));
        heart.beat();
        assert_eq!(heart.buffer.len(), 1);
        assert_eq!(heart.buffer[0].how, "hello");
    }

    #[test]
    fn sensor_experience_returns_buffer() {
        let mut heart = Heart::new(Wit::with_config(
            JoinScheduler::default(),
            None,
            std::time::Duration::from_secs(0),
            "q",
        ));
        heart.feel(Sensation::new(Experience::new("hi")));
        let exps = heart.experience();
        assert_eq!(exps.len(), 1);
        assert!(heart.buffer.is_empty());
    }
}
