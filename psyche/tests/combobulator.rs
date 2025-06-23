use async_trait::async_trait;
use psyche::ling::{Doer, Instruction};
use psyche::{Impression, Stimulus, Summarizer, wit::Episode, wits::CombobulatorSummarizer};

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
    let combo = CombobulatorSummarizer::new(Box::new(Dummy));
    let imp = combo
        .digest(&[Impression::new(
            vec![Stimulus::new(Episode {
                summary: "Pete looked around.".into(),
            })],
            "",
            None::<String>,
        )])
        .await
        .unwrap();
    assert_eq!(imp.stimuli[0].what, "All clear.");
    assert_eq!(imp.summary, "All clear.");
}
