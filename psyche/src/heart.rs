use crate::{Experience, Scheduler, Sensation, Sensor, Wit};

/// Minimal heart managing a single quick wit.
///
/// `Heart` implements [`Sensor`], forwarding sensations to the quick wit. Call
/// [`Heart::beat`] to process queued experiences and update the current instant.
pub struct Heart<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// First layer reflecting raw experiences.
    pub quick: Wit<S>,
    /// Latest experience from the quick wit.
    pub instant: Option<Experience>,
    /// Running count of beats executed.
    pub beat: u64,
}

impl<S> Heart<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// Create a new heart from a quick wit.
    pub fn new(quick: Wit<S>) -> Self {
        Self {
            quick,
            instant: None,
            beat: 0,
        }
    }

    /// Reference to the quick wit.
    pub fn quick(&self) -> Option<&Wit<S>> {
        Some(&self.quick)
    }

    /// Mutable reference to the quick wit.
    pub fn quick_mut(&mut self) -> Option<&mut Wit<S>> {
        Some(&mut self.quick)
    }

    /// Advance the heart one beat.
    ///
    /// The beat counter increments first and then the quick wit processes its
    /// queued experiences. The resulting experience becomes the latest
    /// `instant`.
    pub fn beat(&mut self) {
        self.beat = self.beat.wrapping_add(1);
        if let Some(inst) = self.quick.tick() {
            self.instant = Some(inst);
        }
    }
}

impl<S> Sensor for Heart<S>
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
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JoinScheduler, Sensation, Sensor};

    #[test]
    fn beat_processes_quick() {
        let make = || Wit::with_config(JoinScheduler::default(), None, "w");
        let mut heart = Heart::new(make());
        heart.feel(Sensation::new(Experience::new("hi")));
        heart.beat();
        assert_eq!(heart.beat, 1);
        assert_eq!(heart.instant.as_ref().unwrap().how, "hi");
        assert_eq!(heart.quick.memory.all().len(), 1);
    }

    #[test]
    fn beat_runs_even_when_idle() {
        let make = || Wit::with_config(JoinScheduler::default(), None, "w");
        let mut heart = Heart::new(make());
        heart.beat();
        assert_eq!(heart.beat, 1);
        assert!(heart.instant.is_none());
    }
}
