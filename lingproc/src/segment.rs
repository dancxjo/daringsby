//! Stream segmentation helpers.
//!
//! The [`sentence_stream`] function uses the [`pragmatic_segmenter`] crate to
//! detect sentence boundaries in streaming LLM output. The first sentence is
//! yielded only after at least two complete sentences have been seen to reduce
//! the risk of false completions.
//!
//! The [`word_stream`] function splits the input into Unicode word boundaries.

use crate::segmenter::shared_segmenter;
use futures::{Stream, StreamExt};
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;

/// Yield sentences from the `input` stream.
///
/// # Examples
/// ```
/// use lingproc::sentence_stream;
/// use tokio_stream::iter;
/// use futures::StreamExt;
///
/// let text = "David E. Sanger covers the Trump administration and a range of national security issues. He has been a Times journalist for more than four decades and has written four books on foreign policy and national security challenges.";
/// let mut stream = Box::pin(sentence_stream(iter(vec![Ok::<String, ()>(text.to_string())])));
///
/// futures::executor::block_on(async {
///     assert_eq!(
///         futures::StreamExt::next(&mut stream).await.unwrap().unwrap(),
///         "David E. Sanger covers the Trump administration and a range of national security issues. "
///     );
///     assert_eq!(
///         futures::StreamExt::next(&mut stream).await.unwrap().unwrap(),
///         "He has been a Times journalist for more than four decades and has written four books on foreign policy and national security challenges."
///     );
/// });
/// ```
use crate::types::TextStream;

pub fn sentence_stream<S, E>(input: S) -> TextStream<E>
where
    S: Stream<Item = Result<String, E>> + Unpin + Send + 'static,
{
    use futures::stream::unfold;
    let buf = String::new();
    let leftover = String::new();
    let pending: VecDeque<String> = VecDeque::new();

    let stream = unfold(
        (input, buf, leftover, pending),
        |(mut input, mut buf, mut leftover, mut pending)| async move {
            loop {
                if pending.len() > 1 {
                    if let Some(next) = pending.pop_front() {
                        return Some((Ok(next), (input, buf, leftover, pending)));
                    }
                }
                match input.next().await {
                    Some(Ok(chunk)) => {
                        buf.push_str(&leftover);
                        buf.push_str(&chunk);
                        let mut segments: Vec<String> = shared_segmenter()
                            .segment(&buf)
                            .map(|s| s.to_string())
                            .collect();
                        if !segments.is_empty() {
                            leftover = segments.pop().unwrap();
                            for s in segments {
                                pending.push_back(s);
                            }
                        }
                        buf.clear();
                    }
                    Some(Err(e)) => return Some((Err(e), (input, buf, leftover, pending))),
                    None => {
                        if !leftover.is_empty() {
                            pending.push_back(std::mem::take(&mut leftover));
                        }
                        if let Some(s) = pending.pop_front() {
                            return Some((Ok(s), (input, buf, leftover, pending)));
                        }
                        return None;
                    }
                }
            }
        },
    );
    Box::pin(stream)
}

/// Split the `input` stream into words.
///
/// # Examples
/// ```
/// use lingproc::word_stream;
/// use tokio_stream::iter;
/// use futures::StreamExt;
///
/// let mut words = Box::pin(word_stream(iter(vec![Ok::<String, ()>("Hello 😊 world".to_string())])));
/// futures::executor::block_on(async {
///     assert_eq!(futures::StreamExt::next(&mut words).await.unwrap().unwrap(), "Hello");
///     assert_eq!(futures::StreamExt::next(&mut words).await.unwrap().unwrap(), "world");
///     assert!(futures::StreamExt::next(&mut words).await.is_none());
/// });
/// ```
pub fn word_stream<S, E>(input: S) -> TextStream<E>
where
    S: Stream<Item = Result<String, E>> + Unpin + Send + 'static,
{
    use futures::stream::unfold;
    let buf = String::new();
    let pending: VecDeque<String> = VecDeque::new();

    let stream = unfold(
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
    );
    Box::pin(stream)
}
