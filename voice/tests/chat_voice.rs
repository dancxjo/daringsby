use voice::{ChatVoice, VoiceAgent, model::MockModelClient};

#[tokio::test]
async fn voice_updates_conversation() {
    let llm = MockModelClient::new(vec!["ok".into()], vec![]);
    let voice = ChatVoice::new(llm, "mock", 3);
    voice.receive_user("hi");
    let out = voice.narrate("test").await;
    assert_eq!(out.think.content, "");
    assert_eq!(out.say.unwrap().content, "ok");
}

#[tokio::test]
async fn parses_think_silently() {
    let llm = MockModelClient::new(vec!["hi <think-silently>secret</think-silently>".into()], vec![]);
    let voice = ChatVoice::new(llm, "mock", 3);
    voice.receive_user("yo");
    let out = voice.narrate("ctx").await;
    assert_eq!(out.think.content, "secret");
    assert_eq!(out.say.unwrap().content, "hi");
}
