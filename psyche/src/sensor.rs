use crate::{Experience, Sensation};

/// Something that can transform a [`Sensation`] into an [`Experience`].
///
/// # Examples
/// ```
/// use psyche::{Experience, Sensation, Sensor};
/// struct Echo { last: Option<String> };
/// impl Sensor for Echo {
///     type Input = String;
///     fn feel(&mut self, s: Sensation<Self::Input>) {
///         self.last = Some(s.what);
///     }
///     fn experience(&mut self) -> Vec<Experience> {
///         vec![Experience::new(self.last.take().unwrap())]
///     }
/// }
/// let mut sensor = Echo { last: None };
/// sensor.feel(Sensation::new("hello".to_string()));
/// let exps = sensor.experience();
/// assert_eq!(exps[0].how, "hello");
/// ```
pub trait Sensor {
    /// Type of data this sensor accepts.
    type Input;

    /// Record a sensation for later interpretation.
    fn feel(&mut self, sensation: Sensation<Self::Input>);

    /// Produce an experience from recorded sensations.
    fn experience(&mut self) -> Vec<Experience>;
}
