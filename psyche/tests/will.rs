use async_trait::async_trait;
use psyche::ling::{Doer, Instruction};
use psyche::{Impression, Stimulus, Summarizer, WillSummarizer};

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
        Ok("Do it".to_string())
    }
}

#[tokio::test]
async fn returns_decision_impression() {
    let will = WillSummarizer::new(Box::new(Dummy));
    let imp = will
        .digest(&[Impression::new(
            vec![Stimulus::new("now".to_string())],
            "",
            None::<String>,
        )])
        .await
        .unwrap();
    assert_eq!(imp.stimuli[0].what, "Do it");
    assert_eq!(imp.summary, "Do it");
}
