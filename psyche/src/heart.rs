use crate::{Experience, Scheduler, Sensation, Sensor, Wit};

/// Stack of wits from fond (index 0) to quick (last index).
///
/// `Heart` implements [`Sensor`] so experiences can be fed into the fond wit
/// via [`Sensor::feel`] and processed through each wit using
/// [`Sensor::experience`].
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

    /// Reference to the quick (last wit).
    pub fn quick(&self) -> Option<&W> {
        self.wits.last()
    }

    /// Mutable reference to the quick (last wit).
    pub fn quick_mut(&mut self) -> Option<&mut W> {
        self.wits.last_mut()
    }
}

impl<S> Heart<Wit<S>>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    /// Run ticks in serial until no wit produces output.
    pub fn run_serial(&mut self) -> Option<Experience> {
        loop {
            let mut progressed = false;
            let mut last_output = None;
            for i in 0..self.wits.len() {
                let outputs = self.wits[i].experience();
                if !outputs.is_empty() {
                    progressed = true;
                }
                for exp in outputs {
                    if let Some(next) = self.wits.get_mut(i + 1) {
                        next.feel(Sensation::new(exp));
                    } else {
                        last_output = Some(exp);
                    }
                }
            }
            if !progressed {
                return last_output;
            }
        }
    }

    /// Continuously run ticks respecting each wit's interval.
    pub fn run_scheduled(&mut self, cycles: usize) -> Option<Experience> {
        use std::{
            thread,
            time::{Duration, Instant},
        };
        log::info!("running scheduled for {cycles} cycles");
        let mut completed = 0usize;
        let mut last_output = None;
        while completed < cycles {
            let now = Instant::now();
            let mut next_wait: Option<Duration> = None;
            for i in 0..self.wits.len() {
                let elapsed = now.duration_since(self.wits[i].last_tick);
                if elapsed >= self.wits[i].interval {
                    self.wits[i].last_tick = now;
                    let outputs = self.wits[i].experience();
                    for exp in outputs {
                        if let Some(next) = self.wits.get_mut(i + 1) {
                            next.feel(Sensation::new(exp));
                        } else {
                            last_output = Some(exp);
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
                return last_output;
            }
        }
        last_output
    }
}

impl<S> Sensor for Heart<Wit<S>>
where
    S: Scheduler,
    S::Output: Clone + Into<String>,
{
    type Input = Experience;

    fn feel(&mut self, sensation: Sensation<Self::Input>) {
        let msg = sensation.what.how.clone();
        log::info!("heart feel to fond: {}", msg);
        if let Some(first) = self.wits.first_mut() {
            first.feel(sensation);
        }
    }

    fn experience(&mut self) -> Vec<Experience> {
        use std::time::Instant;
        let mut outputs = Vec::new();
        for i in 0..self.wits.len() {
            let now = Instant::now();
            let elapsed = now.duration_since(self.wits[i].last_tick);
            if elapsed < self.wits[i].interval {
                continue;
            }
            self.wits[i].last_tick = now;
            let wit_outputs = {
                let wit = &mut self.wits[i];
                log::trace!("wit {i} experience");
                wit.experience()
            };
            for exp in wit_outputs {
                if let Some(next) = self.wits.get_mut(i + 1) {
                    next.feel(Sensation::new(exp));
                } else {
                    outputs.push(exp);
                }
            }
        }
        outputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JoinScheduler, Sensation, Sensor, Wit};

    #[test]
    fn heart_flows_between_wits() {
        let w1 = Wit::with_config(
            JoinScheduler::default(),
            None,
            std::time::Duration::from_secs(0),
            "p1",
        );
        let w2 = Wit::with_config(
            JoinScheduler::default(),
            None,
            std::time::Duration::from_secs(0),
            "p2",
        );
        let mut heart = Heart::new(vec![w1, w2]);
        heart.feel(Sensation::new(Experience::new("hello")));
        heart.feel(Sensation::new(Experience::new("world")));
        let _ = heart.experience();
        let _ = heart.experience();
        assert_eq!(heart.wits[0].memory.all().len(), 1);
        assert_eq!(heart.wits[1].memory.all()[0].what, "hello world");
    }

    #[test]
    fn heart_helpers_and_scheduled() {
        use std::time::Duration;
        let w1 = Wit::with_config(
            JoinScheduler::default(),
            Some("fond".to_string()),
            Duration::from_millis(1),
            "fond",
        );
        let w2 = Wit::with_config(
            JoinScheduler::default(),
            Some("quick".to_string()),
            Duration::from_millis(1),
            "quick",
        );
        let mut heart = Heart::new(vec![w1, w2]);
        assert!(heart.fond().is_some());
        assert!(heart.quick().is_some());
        heart.feel(Sensation::new(Experience::new("hello")));
        heart.feel(Sensation::new(Experience::new("world")));
        let _ = heart.run_scheduled(2);
        assert!(!heart.quick().unwrap().memory.all().is_empty());
    }

    #[test]
    fn run_serial_processes_until_idle() {
        let w1 = Wit::with_config(
            JoinScheduler::default(),
            None,
            std::time::Duration::from_secs(0),
            "r1",
        );
        let w2 = Wit::with_config(
            JoinScheduler::default(),
            None,
            std::time::Duration::from_secs(0),
            "r2",
        );
        let mut heart = Heart::new(vec![w1, w2]);
        heart.feel(Sensation::new(Experience::new("hello")));
        let _ = heart.run_serial();
        assert_eq!(heart.wits[0].memory.all().len(), 1);
        assert!(!heart.wits[1].memory.all().is_empty());
    }

    #[test]
    fn heart_flows_across_three_wits() {
        use std::time::Duration;
        let w1 = Wit::with_config(JoinScheduler::default(), None, Duration::from_secs(0), "r1");
        let w2 = Wit::with_config(JoinScheduler::default(), None, Duration::from_secs(0), "r2");
        let w3 = Wit::with_config(JoinScheduler::default(), None, Duration::from_secs(0), "r3");
        let mut heart = Heart::new(vec![w1, w2, w3]);
        heart.feel(Sensation::new(Experience::new("a")));
        heart.feel(Sensation::new(Experience::new("b")));
        let _ = heart.run_serial();
        assert_eq!(heart.wits[0].memory.all()[0].what, "a b");
        assert_eq!(heart.wits[2].memory.all()[0].what, "a b");
    }
}
