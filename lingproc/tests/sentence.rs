use lingproc::sentence_pipe;
use rxrust::prelude::*;
use std::{cell::RefCell, rc::Rc};

#[test]
fn splits_sentences_with_buffering() {
    let (mut input, out) = sentence_pipe();
    let got = Rc::new(RefCell::new(Vec::new()));
    let got_clone = got.clone();
    out.subscribe(move |s: String| got_clone.borrow_mut().push(s));

    input.next("Hello world.".to_string());
    input.next(" How are you? I'm fine.".to_string());
    assert_eq!(got.borrow().len(), 1);
    assert_eq!(got.borrow()[0], "Hello world.");

    input.next(" Thanks.".to_string());
    assert_eq!(got.borrow().len(), 2);
    assert_eq!(got.borrow()[1], "How are you?");
}
