//! Reactive text processing utilities.
//!
//! This crate provides stream-based helpers built on [`rxrust`].
//! The functions emit sentences or words as soon as they can be
//! determined reliably. See the tests for usage examples.

use rxrust::prelude::*;
use sentence_segmentation::processor;
use std::{cell::RefCell, rc::Rc};
use unicode_segmentation::UnicodeSegmentation;

/// Create a pair of subjects for streaming sentence segmentation.
///
/// Feed text fragments into the returned `Subject`. Every time at
/// least two complete sentences are present in the internal buffer,
/// the first is emitted on the output `Subject`.
///
/// ```rust,ignore
/// use lingproc::sentence_pipe;
/// use rxrust::prelude::*;
///
/// let (mut input, out) = sentence_pipe();
/// let mut got = Vec::new();
/// out.subscribe(|s: String| got.push(s));
/// input.next("Hello world.".to_string());
/// input.next(" How are you?".to_string());
/// assert_eq!(got, vec!["Hello world.".to_string()]);
/// ```
use std::convert::Infallible;

pub fn sentence_pipe() -> (
    Subject<'static, String, Infallible>,
    Subject<'static, String, Infallible>,
) {
    let mut input: Subject<'static, String, Infallible> = Subject::default();
    let mut output: Subject<'static, String, Infallible> = Subject::default();
    let buffer = Rc::new(RefCell::new(String::new()));
    let mut out_clone = output.clone();
    let buf_clone = buffer.clone();
    input.clone().subscribe(move |chunk: String| {
        let mut buf = buf_clone.borrow_mut();
        buf.push_str(&chunk);
        let segs = processor::english(&buf);
        if segs.len() >= 2 {
            let first = segs[0].clone();
            out_clone.next(first.clone());
            if let Some(pos) = buf.find(&first) {
                let rest = buf[pos + first.len()..].to_string();
                *buf = rest;
            } else {
                buf.clear();
            }
        }
    });
    (input, output)
}

/// Create a pair of subjects for streaming word segmentation.
///
/// Feed text fragments into the returned `Subject`. Complete words
/// are emitted on the output `Subject` as soon as they are recognized.
pub fn word_pipe() -> (
    Subject<'static, String, Infallible>,
    Subject<'static, String, Infallible>,
) {
    let mut input: Subject<'static, String, Infallible> = Subject::default();
    let mut output: Subject<'static, String, Infallible> = Subject::default();
    let buffer = Rc::new(RefCell::new(String::new()));
    let mut out_clone = output.clone();
    let buf_clone = buffer.clone();
    input.clone().subscribe(move |chunk: String| {
        let mut buf = buf_clone.borrow_mut();
        buf.push_str(&chunk);
        let mut last = 0;
        for (idx, word) in buf.unicode_word_indices() {
            if idx + word.len() == buf.len()
                && buf
                    .chars()
                    .last()
                    .map(|c| c.is_alphanumeric())
                    .unwrap_or(false)
            {
                break;
            }
            out_clone.next(word.to_string());
            last = idx + word.len();
        }
        if last > 0 {
            let rest = buf[last..].to_string();
            *buf = rest;
        }
    });
    (input, output)
}
