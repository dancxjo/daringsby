use async_trait::async_trait;
use psyche::wit::Summarizer;
use psyche::{Impression, Prehension, Wit};

#[derive(Default)]
struct MoodSummarizer;

#[async_trait]
impl Summarizer<String, String> for MoodSummarizer {
    async fn digest(&self, inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        let joined = inputs
            .iter()
            .map(|i| i.raw_data.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let summary = if joined.contains("smiling")
            && joined.contains("frowning")
            && joined.contains("Travis")
        {
            "Travis suddenly becomes sad".to_string()
        } else {
            joined.clone()
        };
        Ok(Impression::new(summary.clone(), Some(joined), summary))
    }
}

#[tokio::test]
async fn prehension_summarizes_buffer() {
    let wit = Prehension::new(MoodSummarizer::default());
    wit.observe(Impression::new(
        "",
        None::<String>,
        "I see a man smiling".to_string(),
    ))
    .await;
    wit.observe(Impression::new(
        "",
        None::<String>,
        "I recognize Travis's face".to_string(),
    ))
    .await;
    wit.observe(Impression::new(
        "",
        None::<String>,
        "I see a man frowning".to_string(),
    ))
    .await;
    let result = wit.tick().await;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].headline, "Travis suddenly becomes sad");
}
