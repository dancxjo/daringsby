use crate::{Experience, Scheduler, Sensation};

/// Join all experience descriptions together.
#[derive(Default)]
pub struct JoinScheduler;

impl Scheduler for JoinScheduler {
    type Output = String;

    fn schedule(&mut self, _prompt: &str, batch: Vec<Experience>) -> Option<Sensation<String>> {
        if batch.is_empty() {
            return None;
        }
        let text = batch
            .into_iter()
            .map(|e| e.how)
            .collect::<Vec<_>>()
            .join(" ");
        Some(Sensation::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sensor;

    struct Echo {
        last: Option<String>,
    }

    impl Sensor for Echo {
        type Input = String;
        fn feel(&mut self, s: Sensation<Self::Input>) {
            self.last = Some(s.what);
        }
        fn experience(&mut self) -> Vec<Experience> {
            vec![Experience::new(self.last.take().unwrap())]
        }
    }

    #[test]
    fn echo_sensor() {
        let mut sensor = Echo { last: None };
        sensor.feel(Sensation::new("hi".to_string()));
        let exps = sensor.experience();
        assert_eq!(exps[0].how, "hi");
    }
}
