use voice::conversation::{Conversation, Role};

#[test]
fn tail_truncates() {
    let mut c = Conversation::new(2);
    c.push(Role::User, "hi");
    c.push(Role::Assistant, "yo");
    c.push(Role::User, "bye");
    assert_eq!(c.tail().len(), 2);
    assert_eq!(c.tail()[0].content, "yo");
}
