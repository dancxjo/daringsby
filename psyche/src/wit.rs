use crate::{Experience, Memory, Scheduler, Sensation, Sensor, narrative_prompt};

/// Processes queued experiences when ticked by the `Heart`.
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
    /// Additional context inserted into the prompt on each tick.
    pub context: String,
}

impl<S> Wit<S>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// Create a new wit from a scheduler with an optional name.
    pub fn new(scheduler: S, prompt: impl Into<String>) -> Self {
        Self::with_config(scheduler, None, prompt)
    }

    /// Create a new wit with a custom name.
    pub fn with_config(scheduler: S, name: Option<String>, prompt: impl Into<String>) -> Self {
        Self {
            scheduler,
            queue: Vec::new(),
            memory: Memory::new(),
            name,
            prompt: prompt.into(),
            context: String::new(),
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

    /// Update the context string used when processing experiences.
    pub fn set_context(&mut self, ctx: impl Into<String>) {
        self.context = ctx.into();
    }

    /// Process queued sensations if any are waiting.
    ///
    /// ```
    /// use psyche::{Wit, JoinScheduler, Experience, Sensation, Sensor};
    /// let mut wit = Wit::with_config(JoinScheduler::default(), None, "tick");
    /// wit.feel(Sensation::new(Experience::new("hello")));
    /// assert!(wit.tick().is_some());
    /// ```
    pub fn tick(&mut self) -> Option<Experience> {
        self.process()
    }

    /// Process queued sensations into an experience using the scheduler.
    fn process(&mut self) -> Option<Experience> {
        let batch = std::mem::take(&mut self.queue);
        if batch.is_empty() {
            return None;
        }
        log::info!("processing {} queued", batch.len());

        let full_prompt = if self.context.is_empty() {
            format!("{} Pete is an artificial consciousness.", self.prompt)
        } else {
            format!(
                "{} Pete is an artificial consciousness. Here's his present situation: {}",
                self.prompt, self.context
            )
        };
        let instruction = narrative_prompt(&full_prompt, &batch);
        let sensation = self.scheduler.schedule(&instruction, batch)?;
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
        match self.tick() {
            Some(exp) => vec![exp],
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JoinScheduler;

    #[test]
    fn tick_clears_queue_and_returns_output() {
        let mut wit = Wit::with_config(JoinScheduler::default(), None, "queue");
        wit.feel(Sensation::new(Experience::new("hello")));
        assert_eq!(wit.queue_len(), 1);
        let exp = wit.tick().unwrap();
        assert_eq!(exp.how, "hello");
        assert_eq!(wit.queue_len(), 0);
    }
}
