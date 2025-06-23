use async_trait::async_trait;
use lingproc::Instruction;
use psyche::traits::Doer;
use psyche::{Impression, Stimulus, wits::Combobulator};
use std::sync::Arc;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
        Ok("All clear.".to_string())
    }
}

#[tokio::test]
async fn returns_awareness_impression() {
    let combo = Combobulator::new(Arc::new(Dummy));
    let imp = combo
        .digest(&[Impression::new(
            vec![Stimulus::new("Pete looked around.".to_string())],
            "",
            None::<String>,
        )])
        .await
        .unwrap();
    assert_eq!(imp.stimuli[0].what, "All clear.");
    assert_eq!(imp.summary, "All clear.");
}
