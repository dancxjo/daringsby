use async_trait::async_trait;
use psyche::wit::Summarizer;
use psyche::{Impression, Prehension, Stimulus, Wit};

#[derive(Default)]
struct MoodSummarizer;

#[async_trait]
impl Summarizer<String, String> for MoodSummarizer {
    async fn digest(&self, inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        let joined = inputs
            .iter()
            .flat_map(|i| i.stimuli.iter().map(|s| s.what.as_str()))
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
        Ok(Impression::new(
            vec![Stimulus::new(summary.clone())],
            summary,
            Some(joined),
        ))
    }
}

#[tokio::test]
async fn prehension_summarizes_buffer() {
    let wit = Prehension::new(MoodSummarizer::default());
    wit.observe(Impression::new(
        vec![Stimulus::new("I see a man smiling".to_string())],
        "",
        None::<String>,
    ))
    .await;
    wit.observe(Impression::new(
        vec![Stimulus::new("I recognize Travis's face".to_string())],
        "",
        None::<String>,
    ))
    .await;
    wit.observe(Impression::new(
        vec![Stimulus::new("I see a man frowning".to_string())],
        "",
        None::<String>,
    ))
    .await;
    let result = wit.tick().await;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].summary, "Travis suddenly becomes sad");
}
