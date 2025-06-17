use async_trait::async_trait;
use psyche::ling::{Doer, Instruction};
use psyche::{Heart, Impression, Summarizer};

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
        Ok("😊".to_string())
    }
}

#[tokio::test]
async fn returns_emoji_impression() {
    let heart = Heart::new(Box::new(Dummy));
    let imp = heart
        .digest(&[Impression {
            headline: "".into(),
            details: None,
            raw_data: "hello".to_string(),
        }])
        .await
        .unwrap();
    assert_eq!(imp.raw_data, "😊");
    assert_eq!(imp.headline, "😊");
}
