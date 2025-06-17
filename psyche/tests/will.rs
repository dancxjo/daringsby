use psyche::ling::{Chatter, Message};
use psyche::{Will, Wit};
use tokio_stream::once;

#[derive(Clone)]
struct Dummy;

#[async_trait::async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _p: &str, _h: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
        Ok(Box::pin(once(Ok("Do it".to_string()))))
    }
}

#[tokio::test]
async fn returns_decision_impression() {
    let will = Will::new(Box::new(Dummy));
    let imp = will.process("now".to_string()).await;
    assert_eq!(imp.raw_data, "Do it");
    assert_eq!(imp.headline, "Do it");
}
