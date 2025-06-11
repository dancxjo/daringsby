use crate::{Heart, Scheduler, Sensation, Sensor, Wit, bus};

/// Central entity combining a [`Heart`] with a set of sensors.
///
/// Sensors convert [`bus::Event`]s into experiences that are queued on the
/// quick wit.
///
/// # Examples
/// ```
/// use psyche::{bus::Event, sensors::{ChatSensor, ConnectionSensor}, JoinScheduler, Sensor, Psyche};
/// let make = || JoinScheduler::default();
/// let external_sensors: Vec<Box<dyn Sensor<Input = Event> + Send + Sync>> = vec![
///     Box::new(ChatSensor::default()),
///     Box::new(ConnectionSensor::default()),
/// ];
/// let mut psyche = Psyche::new(make, external_sensors);
/// use std::net::SocketAddr;
/// psyche.process_event(Event::Connected("127.0.0.1:1".parse().unwrap()));
/// psyche.heart.run_serial();
/// assert_eq!(psyche.heart.quick().unwrap().memory.all().len(), 1);
/// ```
pub struct Psyche<Sched>
where
    Sched: Scheduler,
    Sched::Output: Clone + Into<String>,
{
    /// Internal heart managing wits.
    pub heart: Heart<Wit<Sched>>,
    pub(crate) external_sensors: Vec<Box<dyn Sensor<Input = bus::Event> + Send + Sync>>,
}

impl<Sched> Psyche<Sched>
where
    Sched: Scheduler,
    Sched::Output: Clone + Into<String>,
{
    /// Create a new psyche with the standard set of wits.
    ///
    /// `scheduler_factory` is called once per wit, allowing callers to
    /// configure the underlying scheduler implementation.
    pub fn new<F>(
        mut scheduler_factory: F,
        external_sensors: Vec<Box<dyn Sensor<Input = bus::Event> + Send + Sync>>,
    ) -> Self
    where
        F: FnMut() -> Sched,
    {
        use std::time::Duration;
        let heart = Heart::new(vec![
            Wit::with_config(
                scheduler_factory(),
                Some("fond".into()),
                Duration::from_secs(1),
                "fond",
            ),
            Wit::with_config(
                scheduler_factory(),
                Some("wit2".into()),
                Duration::from_secs(2),
                "wit2",
            ),
            Wit::with_config(
                scheduler_factory(),
                Some("wit3".into()),
                Duration::from_secs(4),
                "wit3",
            ),
            Wit::with_config(
                scheduler_factory(),
                Some("quick".into()),
                Duration::from_secs(8),
                "quick",
            ),
        ]);
        Self {
            heart,
            external_sensors,
        }
    }

    /// Create a psyche from a prebuilt [`Heart`].
    pub fn with_heart(
        heart: Heart<Wit<Sched>>,
        external_sensors: Vec<Box<dyn Sensor<Input = bus::Event> + Send + Sync>>,
    ) -> Self {
        Self {
            heart,
            external_sensors,
        }
    }

    /// Feed an event through all sensors and push resulting experiences to the quick.
    pub fn process_event(&mut self, evt: bus::Event) {
        let sensation = Sensation::new(evt);
        for sensor in &mut self.external_sensors {
            sensor.feel(sensation.clone());
            for exp in sensor.experience() {
                if let Some(quick) = self.heart.quick_mut() {
                    quick.feel(Sensation::new(exp));
                }
            }
        }
    }
}
