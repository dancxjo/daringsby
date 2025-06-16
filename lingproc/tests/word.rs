use lingproc::word_pipe;
use rxrust::prelude::*;
use std::{cell::RefCell, rc::Rc};

#[test]
fn splits_words_streaming() {
    let (mut input, out) = word_pipe();
    let got = Rc::new(RefCell::new(Vec::new()));
    let got_clone = got.clone();
    out.subscribe(move |w: String| got_clone.borrow_mut().push(w));

    input.next("hello wor".to_string());
    assert_eq!(*got.borrow(), vec!["hello".to_string()]);

    input.next("ld friend".to_string());
    assert_eq!(
        *got.borrow(),
        vec!["hello".to_string(), "world".to_string()]
    );
    input.next(" ".to_string());
    assert_eq!(
        *got.borrow(),
        vec![
            "hello".to_string(),
            "world".to_string(),
            "friend".to_string()
        ]
    );
}
