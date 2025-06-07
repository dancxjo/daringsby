use voice::{model::MockModelClient, stream::StreamingVoiceAgent, ChatVoice};

#[tokio::test]
async fn partial_delegates_to_narrate() {
    let client = MockModelClient::new(vec!["hi".into()], vec![]);
    let voice = ChatVoice::new(client, "m", 1);
    let out = StreamingVoiceAgent::narrate_partial(&voice, "ctx").await;
    assert!(out.think.content.is_empty());
}
