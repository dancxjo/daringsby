use async_trait::async_trait;
use psyche::{Impression, Wit};
use std::sync::Arc;

#[derive(Debug)]
struct DummyWit;

#[async_trait]
impl Wit<String, String> for DummyWit {
    async fn digest(&self, _inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        Ok(Impression::new(
            "Dummy headline",
            Some("This is a dummy impression."),
            "raw-dummy-data".to_string(),
        ))
    }
}

#[tokio::test]
async fn wit_should_generate_impression() {
    let wit = DummyWit;
    let result = wit.digest(&[]).await.unwrap();
    assert_eq!(result.headline, "Dummy headline");
    assert_eq!(
        result.details.as_deref(),
        Some("This is a dummy impression.")
    );
    assert_eq!(result.raw_data, "raw-dummy-data");
}

#[tokio::test]
async fn multiple_wits_should_independently_generate_impressions() {
    #[derive(Debug)]
    struct AnotherDummyWit;

    #[async_trait]
    impl Wit<String, String> for AnotherDummyWit {
        async fn digest(
            &self,
            _inputs: &[Impression<String>],
        ) -> anyhow::Result<Impression<String>> {
            Ok(Impression::new(
                "Another headline",
                Some("Another impression."),
                "another-raw".to_string(),
            ))
        }
    }

    let wits: Vec<Arc<dyn Wit<String, String>>> =
        vec![Arc::new(DummyWit), Arc::new(AnotherDummyWit)];

    for wit in wits {
        let result = wit.digest(&[]).await.unwrap();
        assert!(!result.headline.is_empty());
    }
}
