use psyche::Conversation;

#[test]
fn tail_returns_last_n_messages() {
    let mut c = Conversation::default();
    c.add_message_from_user("hello".into());
    c.add_message_from_ai("hi".into());
    c.add_message_from_user("world".into());
    let tail = c.tail(2);
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0].content, "hi");
    assert_eq!(tail[1].content, "world");
}

#[test]
fn tail_does_not_panic_with_large_n() {
    let mut c = Conversation::default();
    c.add_message_from_user("one".into());
    let tail = c.tail(5);
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].content, "one");
}
