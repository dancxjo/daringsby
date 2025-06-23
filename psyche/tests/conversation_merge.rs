use psyche::Conversation;

#[test]
fn consecutive_user_messages_are_spaced_and_trimmed() {
    let mut c = Conversation::default();
    c.add_message_from_user("hi".into());
    c.add_message_from_user("there  ".into());
    assert_eq!(c.all()[0].content, "hi there");
}

#[test]
fn consecutive_ai_messages_are_spaced_and_trimmed() {
    let mut c = Conversation::default();
    c.add_message_from_ai("hello".into());
    c.add_message_from_ai("world  ".into());
    assert_eq!(c.all()[0].content, "hello world");
}
