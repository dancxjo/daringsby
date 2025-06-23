use psyche::psyche::extract_tag;

#[test]
fn parses_well_formed_xml() {
    assert_eq!(
        extract_tag("<take_turn>hi</take_turn>", "take_turn"),
        Some("hi".into())
    );
}

#[test]
fn returns_none_when_closing_missing() {
    assert_eq!(extract_tag("<take_turn>hi</taketurn>", "take_turn"), None);
}

#[test]
fn falls_back_on_malformed_xml() {
    let text = "<take_turn>hi<broken></take_turn>";
    assert_eq!(extract_tag(text, "take_turn"), Some("hi<broken>".into()));
}
