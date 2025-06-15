use async_trait::async_trait;
use psyche::ling::{Chatter, Doer, Message, Vectorizer};

struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, i: &str) -> anyhow::Result<String> {
        Ok(format!("do:{i}"))
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _s: &str, h: &[Message]) -> anyhow::Result<String> {
        Ok(format!("say:{}", h.len()))
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, t: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![t.len() as f32])
    }
}

#[tokio::test]
async fn dummy_traits() {
    let d = Dummy;
    assert_eq!(d.follow("a").await.unwrap(), "do:a");
    let hist = vec![Message::user("hi"), Message::assistant("hey")];
    assert_eq!(d.chat("sys", &hist).await.unwrap(), "say:2");
    assert_eq!(d.vectorize("xyz").await.unwrap(), vec![3.0]);
}
