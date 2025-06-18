//! Stream segmentation helpers.
//!
//! The [`sentence_stream`] function uses the [`pragmatic_segmenter`] crate to
//! detect sentence boundaries in streaming LLM output. The first sentence is
//! yielded only after at least two complete sentences have been seen to reduce
//! the risk of false completions.
//!
//! The [`word_stream`] function splits the input into Unicode word boundaries.

use futures::{Stream, StreamExt};
use pragmatic_segmenter::Segmenter;
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;

/// Yield sentences from the `input` stream.
pub fn sentence_stream<S, E>(input: S) -> impl Stream<Item = Result<String, E>>
where
    S: Stream<Item = Result<String, E>> + Unpin,
{
    use futures::stream::unfold;
    let buf = String::new();
    let seg = Segmenter::new().expect("segmenter init");
    let leftover = String::new();
    let pending: VecDeque<String> = VecDeque::new();

    unfold(
        (input, buf, seg, leftover, pending),
        |(mut input, mut buf, seg, mut leftover, mut pending)| async move {
            loop {
                if pending.len() > 1 {
                    if let Some(next) = pending.pop_front() {
                        return Some((Ok(next), (input, buf, seg, leftover, pending)));
                    }
                }
                match input.next().await {
                    Some(Ok(chunk)) => {
                        buf.push_str(&leftover);
                        buf.push_str(&chunk);
                        let mut segments: Vec<String> =
                            seg.segment(&buf).map(|s| s.to_string()).collect();
                        if !segments.is_empty() {
                            leftover = segments.pop().unwrap();
                            for s in segments {
                                pending.push_back(s);
                            }
                        }
                        buf.clear();
                    }
                    Some(Err(e)) => return Some((Err(e), (input, buf, seg, leftover, pending))),
                    None => {
                        if !leftover.is_empty() {
                            pending.push_back(std::mem::take(&mut leftover));
                        }
                        if let Some(s) = pending.pop_front() {
                            return Some((Ok(s), (input, buf, seg, leftover, pending)));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

/// Split the `input` stream into words.
pub fn word_stream<S, E>(input: S) -> impl Stream<Item = Result<String, E>>
where
    S: Stream<Item = Result<String, E>> + Unpin,
{
    use futures::stream::unfold;
    let buf = String::new();
    let pending: VecDeque<String> = VecDeque::new();

    unfold(
        (input, buf, pending),
        |(mut input, mut buf, mut pending)| async move {
            loop {
                if let Some(word) = pending.pop_front() {
                    return Some((Ok(word), (input, buf, pending)));
                }
                match input.next().await {
                    Some(Ok(chunk)) => {
                        buf.push_str(&chunk);
                        for w in UnicodeSegmentation::unicode_words(buf.as_str()) {
                            pending.push_back(w.to_string());
                        }
                        buf.clear();
                    }
                    Some(Err(e)) => return Some((Err(e), (input, buf, pending))),
                    None => {
                        if !buf.is_empty() {
                            for w in UnicodeSegmentation::unicode_words(buf.as_str()) {
                                pending.push_back(w.to_string());
                            }
                            buf.clear();
                            if let Some(word) = pending.pop_front() {
                                return Some((Ok(word), (input, buf, pending)));
                            }
                        }
                        return None;
                    }
                }
            }
        },
    )
}
