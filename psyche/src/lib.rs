use std::time::SystemTime;

/// Input data captured by the system.
///
/// `Sensation` wraps any data with a timestamp so the moment of
/// perception is remembered.
///
/// # Examples
/// ```
/// use psyche::Sensation;
/// let s = Sensation::new(42);
/// assert_eq!(s.what, 42);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Sensation<T> {
    /// Time when the data was perceived.
    pub when: SystemTime,
    /// Arbitrary data representing the perception.
    pub what: T,
}

impl<T> Sensation<T> {
    /// Create a new sensation stamped with `SystemTime::now()`.
    pub fn new(data: T) -> Self {
        Self {
            when: SystemTime::now(),
            what: data,
        }
    }

    /// Create a sensation with a specified timestamp.
    pub fn with_timestamp(data: T, timestamp: SystemTime) -> Self {
        Self {
            when: timestamp,
            what: data,
        }
    }
}

/// Linguistic interpretation of a sensation.
///
/// `Experience` is meant to be one sentence describing the input.
///
/// # Examples
/// ```
/// use psyche::Experience;
/// let e = Experience::new("I see a cat.");
/// assert_eq!(e.sentence, "I see a cat.");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Experience {
    /// The explanatory sentence.
    pub sentence: String,
}

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
        self.entries.push(s);
    }

    /// Access all recorded sensations.
    pub fn all(&self) -> &[Sensation<T>] {
        &self.entries
    }
}

/// Convert a batch of experiences into a new sensation.
pub trait Scheduler {
    type Output;
    fn schedule(&mut self, batch: Vec<Experience>) -> Option<Sensation<Self::Output>>;
}

/// Join all sentences together.
#[derive(Default)]
pub struct JoinScheduler;

impl Scheduler for JoinScheduler {
    type Output = String;
    fn schedule(&mut self, batch: Vec<Experience>) -> Option<Sensation<String>> {
        if batch.is_empty() {
            return None;
        }
        let text = batch
            .into_iter()
            .map(|e| e.sentence)
            .collect::<Vec<_>>()
            .join(" ");
        Some(Sensation::new(text))
    }
}

/// Timed loop processing experiences.
pub struct Wit<S, P>
where
    S: Scheduler,
    P: Sensor<Input = S::Output>,
    S::Output: Clone,
{
    scheduler: S,
    sensor: P,
    queue: Vec<Experience>,
    pub memory: Memory<S::Output>,
}

impl<S, P> Wit<S, P>
where
    S: Scheduler,
    P: Sensor<Input = S::Output>,
    S::Output: Clone,
{
    /// Create a new wit from a scheduler and sensor.
    pub fn new(scheduler: S, sensor: P) -> Self {
        Self {
            scheduler,
            sensor,
            queue: Vec::new(),
            memory: Memory::new(),
        }
    }

    /// Queue an experience for later processing.
    pub fn push(&mut self, exp: Experience) {
        self.queue.push(exp);
    }

    /// Process queued experiences and return a summary experience.
    pub fn tick(&mut self) -> Option<Experience> {
        let batch = std::mem::take(&mut self.queue);
        if batch.is_empty() {
            return None;
        }
        let sensation = self.scheduler.schedule(batch)?;
        self.memory.remember(sensation.clone());
        self.sensor.feel(sensation)
    }
}

/// Stack of wits from fond (index 0) to focus (last index).
pub struct Heart<W> {
    pub wits: Vec<W>,
}

impl<W> Heart<W> {
    /// Create a heart from a set of wits.
    pub fn new(wits: Vec<W>) -> Self {
        Self { wits }
    }
}

impl<S, P> Heart<Wit<S, P>>
where
    S: Scheduler,
    P: Sensor<Input = S::Output>,
    S::Output: Clone,
{
    /// Push a new experience into the fond.
    pub fn push(&mut self, exp: Experience) {
        if let Some(first) = self.wits.first_mut() {
            first.push(exp);
        }
    }

    /// Run one processing tick across all wits.
    pub fn tick(&mut self) {
        for i in 0..self.wits.len() {
            let output = {
                let wit = &mut self.wits[i];
                wit.tick()
            };
            if let Some(exp) = output {
                if let Some(next) = self.wits.get_mut(i + 1) {
                    next.push(exp);
                }
            }
        }
    }
}

/// Something that can transform a [`Sensation`] into an [`Experience`].
///
/// # Examples
/// ```
/// use psyche::{Experience, Sensation, Sensor};
/// struct Echo;
/// impl Sensor for Echo {
///     type Input = String;
///     fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
///         Some(Experience::new(s.what))
///     }
/// }
/// let mut sensor = Echo;
/// let exp = sensor.feel(Sensation::new("hello".to_string())).unwrap();
/// assert_eq!(exp.sentence, "hello");
/// ```
pub trait Sensor {
    /// Type of data this sensor accepts.
    type Input;

    /// Convert a sensation into an experience, if possible.
    fn feel(&mut self, sensation: Sensation<Self::Input>) -> Option<Experience>;
}

impl Experience {
    /// Create a new experience from a sentence.
    pub fn new(sentence: impl Into<String>) -> Self {
        Self {
            sentence: sentence.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Sensor for Echo {
        type Input = String;
        fn feel(&mut self, s: Sensation<Self::Input>) -> Option<Experience> {
            Some(Experience::new(s.what))
        }
    }

    #[test]
    fn create_sensation() {
        let s = Sensation::new(123u8);
        assert_eq!(s.what, 123);
    }

    #[test]
    fn create_experience() {
        let e = Experience::new("just a test");
        assert_eq!(e.sentence, "just a test");
    }

    #[test]
    fn echo_sensor() {
        let mut sensor = Echo;
        let exp = sensor.feel(Sensation::new("hi".to_string())).unwrap();
        assert_eq!(exp.sentence, "hi");
    }

    #[test]
    fn memory_records() {
        let mut mem = Memory::new();
        mem.remember(Sensation::new(1u8));
        assert_eq!(mem.all().len(), 1);
    }

    #[test]
    fn heart_flows_between_wits() {
        let w1 = Wit::new(JoinScheduler::default(), Echo);
        let w2 = Wit::new(JoinScheduler::default(), Echo);
        let mut heart = Heart::new(vec![w1, w2]);
        heart.push(Experience::new("hello"));
        heart.push(Experience::new("world"));
        heart.tick();
        heart.tick();
        assert_eq!(heart.wits[0].memory.all().len(), 1);
        assert_eq!(heart.wits[1].memory.all()[0].what, "hello world");
    }
}
