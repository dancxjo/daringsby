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

    /// Propagate experiences through all wits updating instant, moment and context.
    pub fn beat(&mut self) {
        if let Some(inst) = self.quick.tick() {
            self.instant = Some(inst.clone());
            self.combobulator.feel(Sensation::new(inst));
        }

        if let Some(mom) = self.combobulator.tick() {
            self.moment = Some(mom.clone());
            self.contextualizer.feel(Sensation::new(mom));
        }

        if let Some(ctx) = self.contextualizer.tick() {
            let c = ctx.how.clone();
            self.context = Some(c.clone());
            self.quick.set_context(c.clone());
            self.combobulator.set_context(c.clone());
            self.contextualizer.set_context(c);
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
}
