use std::time::SystemTime;
use serde::Serialize;

pub mod bus;
pub mod logging;
pub mod sensors;
pub mod server;

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
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sensation<T>
{
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
#[derive(Debug, Clone, PartialEq, Serialize)]
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
        log::info!("remembering sensation");
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

/// Scheduler using an LLM processor to summarize sentences.
pub struct ProcessorScheduler<P> {
    processor: P,
}

impl<P> ProcessorScheduler<P> {
    /// Create a new scheduler wrapping the given processor.
    pub fn new(processor: P) -> Self {
        Self { processor }
    }
}

impl<P> Scheduler for ProcessorScheduler<P>
where
    P: lingproc::Processor + Send + Sync + 'static,
{
    type Output = String;

    fn schedule(&mut self, batch: Vec<Experience>) -> Option<Sensation<String>> {
        use futures::StreamExt;
        use lingproc::{InstructionFollowingTask, Task, TaskOutput};

        if batch.is_empty() {
            return None;
        }

        let instruction = batch
            .into_iter()
            .map(|e| e.sentence)
            .collect::<Vec<_>>()
            .join(" ");

        let task = Task::InstructionFollowing(InstructionFollowingTask {
            instruction,
            images: vec![],
        });

        let rt = tokio::runtime::Runtime::new().ok()?;
        let mut stream = rt.block_on(self.processor.process(task)).ok()?;
        let mut text = String::new();
        while let Some(chunk) = rt.block_on(stream.next()) {
            match chunk.ok()? {
                TaskOutput::TextChunk(t) => text.push_str(&t),
                _ => {}
            }
        }
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
    /// Optional human readable identifier.
    pub name: Option<String>,
    /// Interval between ticks.
    pub interval: std::time::Duration,
    last_tick: std::time::Instant,
}

impl<S, P> Wit<S, P>
where
    S: Scheduler,
    P: Sensor<Input = S::Output>,
    S::Output: Clone,
{
    /// Create a new wit from a scheduler and sensor with default settings.
    pub fn new(scheduler: S, sensor: P) -> Self {
        Self::with_config(scheduler, sensor, None, std::time::Duration::from_secs(1))
    }

    /// Create a new wit with a custom name and tick interval.
    pub fn with_config(
        scheduler: S,
        sensor: P,
        name: Option<String>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            scheduler,
            sensor,
            queue: Vec::new(),
            memory: Memory::new(),
            name,
            interval,
            last_tick: std::time::Instant::now(),
        }
    }

    /// Queue an experience for later processing.
    pub fn push(&mut self, exp: Experience) {
        log::info!("queued experience: {}", exp.sentence);
        self.queue.push(exp);
    }

    /// Process queued experiences and return a summary experience.
    pub fn tick(&mut self) -> Option<Experience> {
        let batch = std::mem::take(&mut self.queue);
        log::info!("processing {} queued", batch.len());
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

    /// Reference to the fond (first wit).
    pub fn fond(&self) -> Option<&W> {
        self.wits.first()
    }

    /// Mutable reference to the fond (first wit).
    pub fn fond_mut(&mut self) -> Option<&mut W> {
        self.wits.first_mut()
    }

    /// Reference to the focus (last wit).
    pub fn focus(&self) -> Option<&W> {
        self.wits.last()
    }

    /// Mutable reference to the focus (last wit).
    pub fn focus_mut(&mut self) -> Option<&mut W> {
        self.wits.last_mut()
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
        log::info!("heart push to fond: {}", exp.sentence);
        if let Some(first) = self.wits.first_mut() {
            first.push(exp);
        }
    }

    /// Run one processing tick across all wits.
    pub fn tick(&mut self) {
        for i in 0..self.wits.len() {
            let output = {
                let wit = &mut self.wits[i];
                log::info!("wit {i} tick");
                wit.tick()
            };
            if let Some(exp) = output {
                if let Some(next) = self.wits.get_mut(i + 1) {
                    next.push(exp);
                }
            }
        }
    }

    /// Continuously run ticks respecting each wit's interval.
    pub fn run_scheduled(&mut self, cycles: usize) {
        use std::{
            thread,
            time::{Duration, Instant},
        };
        log::info!("running scheduled for {cycles} cycles");
        let mut completed = 0usize;
        while completed < cycles {
            let now = Instant::now();
            let mut next_wait: Option<Duration> = None;
            for i in 0..self.wits.len() {
                let elapsed = now.duration_since(self.wits[i].last_tick);
                if elapsed >= self.wits[i].interval {
                    self.wits[i].last_tick = now;
                    let output = self.wits[i].tick();
                    if let Some(exp) = output {
                        if let Some(next) = self.wits.get_mut(i + 1) {
                            next.push(exp);
                        }
                    }
                    completed += 1;
                }
                let remaining = self.wits[i]
                    .interval
                    .checked_sub(elapsed)
                    .unwrap_or_default();
                next_wait = Some(match next_wait {
                    Some(d) => d.min(remaining),
                    None => remaining,
                });
            }
            if let Some(wait) = next_wait {
                if !wait.is_zero() {
                    thread::sleep(wait);
                }
            } else {
                break;
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

/// Central entity combining a [`Heart`] with a set of sensors.
///
/// Sensors convert [`bus::Event`]s into experiences that are queued on the
/// focus wit.
///
/// # Examples
/// ```
/// use psyche::{bus::Event, sensors::{ChatSensor, ConnectionSensor}, Heart, Wit, JoinScheduler, Sensor};
/// struct Echo;
/// impl psyche::Sensor for Echo { type Input = String; fn feel(&mut self, s: psyche::Sensation<String>) -> Option<psyche::Experience> { Some(psyche::Experience::new(s.what)) } }
/// let wit = Wit::new(JoinScheduler::default(), Echo);
/// let sensors: Vec<Box<dyn Sensor<Input = Event> + Send + Sync>> = vec![
///     Box::new(ChatSensor::default()),
///     Box::new(ConnectionSensor::default()),
/// ];
/// let mut psyche = psyche::Psyche::new(Heart::new(vec![wit]), sensors);
/// use std::net::SocketAddr;
/// psyche.process_event(Event::Connected("127.0.0.1:1".parse().unwrap()));
/// psyche.heart.tick();
/// assert_eq!(psyche.heart.wits[0].memory.all().len(), 1);
/// ```
pub struct Psyche<Sched, Percept>
where
    Sched: Scheduler,
    Percept: Sensor<Input = Sched::Output>,
    Sched::Output: Clone,
{
    /// Internal heart managing wits.
    pub heart: Heart<Wit<Sched, Percept>>,
    sensors: Vec<Box<dyn Sensor<Input = bus::Event> + Send + Sync>>,
}

impl<Sched, Percept> Psyche<Sched, Percept>
where
    Sched: Scheduler,
    Percept: Sensor<Input = Sched::Output>,
    Sched::Output: Clone,
{
    /// Create a new psyche from a heart and sensors.
    pub fn new(
        heart: Heart<Wit<Sched, Percept>>,
        sensors: Vec<Box<dyn Sensor<Input = bus::Event> + Send + Sync>>,
    ) -> Self {
        Self { heart, sensors }
    }

    /// Feed an event through all sensors and push resulting experiences to the focus.
    pub fn process_event(&mut self, evt: bus::Event) {
        let sensation = Sensation::new(evt);
        for sensor in &mut self.sensors {
            if let Some(exp) = sensor.feel(sensation.clone()) {
                if let Some(focus) = self.heart.focus_mut() {
                    focus.push(exp);
                }
            }
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

    #[test]
    fn heart_helpers_and_scheduled() {
        use std::time::Duration;
        let w1 = Wit::with_config(
            JoinScheduler::default(),
            Echo,
            Some("fond".to_string()),
            Duration::from_millis(1),
        );
        let w2 = Wit::with_config(
            JoinScheduler::default(),
            Echo,
            Some("focus".to_string()),
            Duration::from_millis(1),
        );
        let mut heart = Heart::new(vec![w1, w2]);
        assert!(heart.fond().is_some());
        assert!(heart.focus().is_some());
        heart.push(Experience::new("hello"));
        heart.push(Experience::new("world"));
        heart.run_scheduled(2);
        assert!(!heart.focus().unwrap().memory.all().is_empty());
    }

    #[test]
    fn processor_scheduler_runs_llm() {
        use async_stream::stream;
        use async_trait::async_trait;
        use futures::{StreamExt, stream::BoxStream};
        use lingproc::{InstructionFollowingTask, Processor, Task, TaskKind, TaskOutput};

        struct MockProcessor;

        #[async_trait]
        impl Processor for MockProcessor {
            fn capabilities(&self) -> Vec<TaskKind> {
                vec![TaskKind::InstructionFollowing]
            }

            async fn process(
                &self,
                task: Task,
            ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
                match task {
                    Task::InstructionFollowing(t) => {
                        let instr = t.instruction;
                        let s = stream! { yield Ok(TaskOutput::TextChunk(format!("processed {instr}"))); };
                        Ok(Box::pin(s))
                    }
                    _ => Err(anyhow::anyhow!("unsupported")),
                }
            }
        }

        let scheduler = ProcessorScheduler::new(MockProcessor);
        let mut wit = Wit::new(scheduler, Echo);
        wit.push(Experience::new("one"));
        wit.push(Experience::new("two"));
        let exp = wit.tick().unwrap();
        assert_eq!(exp.sentence, "processed one two");
        assert_eq!(wit.memory.all()[0].what, "processed one two");
    }
}
