use async_trait::async_trait;
use psyche::ling::{ChatContext, Chatter, Doer, Message, Vectorizer};
use tokio_stream::StreamExt;

struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, i: &str) -> anyhow::Result<String> {
        Ok(format!("do:{i}"))
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, ctx: ChatContext<'_>) -> anyhow::Result<psyche::ling::ChatStream> {
        let msg = format!("say:{}", ctx.history.len());
        Ok(Box::pin(tokio_stream::once(Ok(msg))))
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
    let ctx = ChatContext {
        system_prompt: "sys",
        history: &hist,
        emotion: None,
    };
    let mut stream = d.chat(ctx).await.unwrap();
    let mut res = String::new();
    while let Some(chunk) = stream.next().await.transpose().unwrap() {
        res.push_str(&chunk);
    }
    assert_eq!(res, "say:2");
    assert_eq!(d.vectorize("xyz").await.unwrap(), vec![3.0]);
}
