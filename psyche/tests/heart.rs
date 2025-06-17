use async_trait::async_trait;
use psyche::ling::{ChatContext, Chatter, Message};
use psyche::{Heart, Wit};
use tokio_stream::once;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _: ChatContext<'_>) -> anyhow::Result<psyche::ling::ChatStream> {
        Ok(Box::pin(once(Ok("😊".to_string()))))
    }
}

#[tokio::test]
async fn returns_emoji_impression() {
    let heart = Heart::new(Box::new(Dummy));
    let imp = heart.process("hello".to_string()).await;
    assert_eq!(imp.raw_data, "😊");
    assert_eq!(imp.headline, "😊");
}
