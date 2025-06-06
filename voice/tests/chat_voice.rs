use voice::{ChatVoice, VoiceAgent, model::MockModelClient};

#[tokio::test]
async fn voice_updates_conversation() {
    let llm = MockModelClient::new(vec!["<say>ok</say>".into()], vec![]);
    let voice = ChatVoice::new(llm, "mock", 3);
    voice.receive_user("hi");
    let out = voice.narrate("test").await;
    assert_eq!(out.say.unwrap().content, "ok");
}
