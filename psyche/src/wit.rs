use crate::{Experience, Memory, Scheduler, Sensation, Sensor};

/// Timed loop processing experiences.
pub struct Wit<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    pub(crate) scheduler: S,
    pub(crate) queue: Vec<Experience>,
    pub memory: Memory<S::Output>,
    /// Optional human readable identifier.
    pub name: Option<String>,
    /// Prompt passed to the scheduler when summarizing experiences.
    pub prompt: String,
    /// Interval between ticks.
    pub interval: std::time::Duration,
    pub(crate) last_tick: std::time::Instant,
}

impl<S> Wit<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// Create a new wit from a scheduler with default settings.
    pub fn new(scheduler: S, prompt: impl Into<String>) -> Self {
        Self::with_config(scheduler, None, std::time::Duration::from_secs(1), prompt)
    }

    /// Create a new wit with a custom name and tick interval.
    pub fn with_config(
        scheduler: S,
        name: Option<String>,
        interval: std::time::Duration,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            scheduler,
            queue: Vec::new(),
            memory: Memory::new(),
            name,
            prompt: prompt.into(),
            interval,
            last_tick: std::time::Instant::now(),
        }
    }

    /// Current number of queued experiences.
    ///
    /// ```
    /// use psyche::{Wit, JoinScheduler, Experience, Sensation, Sensor};
    /// let mut wit = Wit::new(JoinScheduler::default(), "prompt");
    /// assert_eq!(wit.queue_len(), 0);
    /// wit.feel(Sensation::new(Experience::new("test")));
    /// assert_eq!(wit.queue_len(), 1);
    /// ```
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Process queued sensations into an experience using the scheduler.
    fn process(&mut self) -> Option<Experience> {
        let batch = std::mem::take(&mut self.queue);
        if batch.is_empty() {
            return None;
        }
        log::info!("processing {} queued", batch.len());

        let sensation = self.scheduler.schedule(&self.prompt, batch)?;
        self.memory.remember(sensation.clone());
        Some(Experience::with_timestamp(sensation.what, sensation.when))
    }
}

impl<S> Sensor for Wit<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    type Input = Experience;

    fn feel(&mut self, sensation: Sensation<Self::Input>) {
        log::info!("queued experience: {}", sensation.what.how);
        self.queue.push(sensation.what);
    }

    fn experience(&mut self) -> Vec<Experience> {
        match self.process() {
            Some(exp) => vec![exp],
            None => Vec::new(),
        }
    }
}
