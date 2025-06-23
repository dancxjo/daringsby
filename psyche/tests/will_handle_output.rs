use async_trait::async_trait;
use lingproc::Instruction;
use psyche::motorcall::{Motor, MotorRegistry};
use psyche::traits::Doer;
use psyche::wits::Will;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[derive(Default)]
struct RecMotor(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Motor for RecMotor {
    async fn execute(&self, _attrs: HashMap<String, String>, content: String) {
        self.0.lock().unwrap().push(content);
    }
}

#[tokio::test]
async fn parses_motor_tags() {
    let mut will = Will::new(psyche::TopicBus::new(8), Arc::new(Dummy));
    let motor = Arc::new(RecMotor::default());
    will.motor_registry_mut().register("move", motor.clone());
    will.handle_llm_output("hello <move speed=\"fast\">go</move>")
        .await;
    let calls = motor.0.lock().unwrap();
    assert_eq!(calls.as_slice(), ["go"]);
}
