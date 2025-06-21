use async_trait::async_trait;
use psyche::{Impression, Stimulus, Summarizer};
use std::sync::Arc;

#[derive(Debug)]
struct DummyWit;

#[async_trait]
impl Summarizer<String, String> for DummyWit {
    async fn digest(&self, _inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        Ok(Impression::new(
            vec![Stimulus::new("raw-dummy-data".to_string())],
            "Dummy headline",
            Some("This is a dummy impression."),
        ))
    }
}

#[tokio::test]
async fn wit_should_generate_impression() {
    let wit = DummyWit;
    let result = wit.digest(&[]).await.unwrap();
    assert_eq!(result.summary, "Dummy headline");
    assert_eq!(result.emoji.as_deref(), Some("This is a dummy impression."));
    assert_eq!(result.stimuli[0].what, "raw-dummy-data");
}

#[tokio::test]
async fn multiple_wits_should_independently_generate_impressions() {
    #[derive(Debug)]
    struct AnotherDummyWit;

    #[async_trait]
    impl Summarizer<String, String> for AnotherDummyWit {
        async fn digest(
            &self,
            _inputs: &[Impression<String>],
        ) -> anyhow::Result<Impression<String>> {
            Ok(Impression::new(
                vec![Stimulus::new("another-raw".to_string())],
                "Another headline",
                Some("Another impression."),
            ))
        }
    }

    let wits: Vec<Arc<dyn Summarizer<String, String>>> =
        vec![Arc::new(DummyWit), Arc::new(AnotherDummyWit)];

    for wit in wits {
        let result = wit.digest(&[]).await.unwrap();
        assert!(!result.summary.is_empty());
    }
}
