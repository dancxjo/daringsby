use async_trait::async_trait;
use psyche::ling::{Doer, Instruction};
use psyche::{Impression, Summarizer, Will};

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
    let will = Will::new(Box::new(Dummy));
    let imp = will
        .digest(&[Impression {
            headline: "".into(),
            details: None,
            raw_data: "now".to_string(),
        }])
        .await
        .unwrap();
    assert_eq!(imp.raw_data, "Do it");
    assert_eq!(imp.headline, "Do it");
}
