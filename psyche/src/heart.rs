use crate::{Experience, Scheduler, Sensation, Sensor, Wit};

/// Multi-layer heart chaining a quick wit, combobulator and contextualizer.
///
/// `Heart` implements [`Sensor`], forwarding sensations to the quick wit. Call
/// [`Heart::beat`] to propagate experiences through the wits and update the
/// current instant, moment and context.
pub struct Heart<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// First layer reflecting raw experiences.
    pub quick: Wit<S>,
    /// Wit buffering instants into moments.
    pub combobulator: Wit<S>,
    /// Wit summarizing moments into context.
    pub contextualizer: Wit<S>,
    /// Latest experience from the quick wit.
    pub instant: Option<Experience>,
    /// Latest experience from the combobulator.
    pub moment: Option<Experience>,
    /// Latest context string from the contextualizer.
    pub context: Option<String>,
    /// Running count of beats executed.
    pub beat: u64,
}

impl<S> Heart<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// Create a new heart from three wits.
    pub fn new(quick: Wit<S>, combobulator: Wit<S>, contextualizer: Wit<S>) -> Self {
        Self {
            quick,
            combobulator,
            contextualizer,
            instant: None,
            moment: None,
            context: None,
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

    /// Milliseconds until the next wit is due for a tick.
    pub fn due_ms(&self) -> u64 {
        self.quick
            .due_ms()
            .min(self.combobulator.due_ms())
            .min(self.contextualizer.due_ms())
    }

    /// Advance the heart one beat running wits based on the beat counter.
    ///
    /// - On even beats the `quick` wit ticks.
    /// - Beats divisible by 7 tick the `combobulator`.
    /// - Beats divisible by 11 tick the `contextualizer`.
    ///
    /// The beat counter starts at zero and increments after each call.
    pub fn beat(&mut self) {
        if self.beat % 2 == 0 {
            if let Some(inst) = self.quick.tick() {
                self.instant = Some(inst.clone());
                self.combobulator.feel(Sensation::new(inst));
            }
        }

        if self.beat % 7 == 0 {
            if let Some(mom) = self.combobulator.tick() {
                self.moment = Some(mom.clone());
                self.contextualizer.feel(Sensation::new(mom));
            }
        }

        if self.beat % 11 == 0 {
            if let Some(ctx) = self.contextualizer.tick() {
                let c = ctx.how.clone();
                self.context = Some(c.clone());
                self.quick.set_context(c.clone());
                self.combobulator.set_context(c.clone());
                self.contextualizer.set_context(c);
            }
        }

        self.beat = self.beat.wrapping_add(1);
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
    fn passes_experiences_through_layers() {
        let make = || {
            Wit::with_config(
                JoinScheduler::default(),
                None,
                std::time::Duration::from_secs(0),
                "w",
            )
        };
        let mut heart = Heart::new(make(), make(), make());
        heart.feel(Sensation::new(Experience::new("hi")));
        heart.beat();
        assert_eq!(heart.instant.as_ref().unwrap().how, "hi");
        assert_eq!(heart.moment.as_ref().unwrap().how, "hi");
        assert_eq!(heart.context.as_deref(), Some("hi"));
        assert_eq!(heart.quick.context, "hi");
        assert_eq!(heart.combobulator.context, "hi");
    }

    #[test]
    fn due_ms_is_min_of_wits() {
        let make = |ms| {
            Wit::with_config(
                JoinScheduler::default(),
                None,
                std::time::Duration::from_millis(ms),
                "w",
            )
        };
        let heart = Heart::new(make(100), make(50), make(200));
        assert!(heart.due_ms() <= 50);
    }

    #[test]
    fn beat_follows_schedule() {
        let make = || {
            Wit::with_config(
                JoinScheduler::default(),
                None,
                std::time::Duration::from_secs(0),
                "w",
            )
        };
        let mut heart = Heart::new(make(), make(), make());
        for i in 0..15 {
            heart.feel(Sensation::new(Experience::new(format!("{i}"))));
            heart.beat();
        }
        assert_eq!(heart.beat, 15);
        assert_eq!(heart.quick.memory.all().len(), 8); // even beats
        assert_eq!(heart.combobulator.memory.all().len(), 3); // multiples of 7
        assert_eq!(heart.contextualizer.memory.all().len(), 2); // multiples of 11
    }
}
