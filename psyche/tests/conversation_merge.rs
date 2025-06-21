use psyche::Conversation;

#[test]
fn consecutive_user_messages_are_spaced_and_trimmed() {
    let mut c = Conversation::default();
    c.add_user("hi".into());
    c.add_user("there  ".into());
    assert_eq!(c.all()[0].content, "hi there");
}
