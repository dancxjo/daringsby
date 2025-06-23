use async_trait::async_trait;
use lingproc::Instruction as LlmInstruction;
use psyche::traits::Doer;
use psyche::wits::Will;
use psyche::{Impression, Instruction, Stimulus, TopicBus, Wit};
use std::sync::Arc;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: LlmInstruction) -> anyhow::Result<String> {
        Ok("<say>Do it</say>".to_string())
    }
}

#[tokio::test]
async fn returns_decision_impression() {
    let bus = TopicBus::new(8);
    let will = Will::new(bus, Arc::new(Dummy));
    will.observe(Impression::new(
        vec![Stimulus::new("now".to_string())],
        "",
        None::<String>,
    ))
    .await;
    let imp = will.tick().await.pop().unwrap();
    assert_eq!(
        imp.stimuli[0].what.instructions[0],
        Instruction::Say {
            voice: None,
            text: "Do it".into()
        }
    );
    assert_eq!(imp.summary, "<say>Do it</say>");
}
