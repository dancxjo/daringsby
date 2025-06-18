use async_trait::async_trait;
use chrono::Utc;
use psyche::ling::{Doer, Instruction};
use psyche::{Impression, Summarizer, wit::Episode, wits::Combobulator};
use uuid::Uuid;

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
    let combo = Combobulator::new(Box::new(Dummy));
    let imp = combo
        .digest(&[Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: "".into(),
            details: None,
            raw_data: Episode {
                summary: "Pete looked around.".into(),
            },
        }])
        .await
        .unwrap();
    assert_eq!(imp.raw_data, "All clear.");
    assert_eq!(imp.headline, "All clear.");
}
