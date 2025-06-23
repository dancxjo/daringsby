//! Stream segmentation helpers.
//!
//! The [`sentence_stream`] function uses the [`pragmatic_segmenter`] crate to
//! detect sentence boundaries in streaming LLM output. The first sentence is
//! yielded only after at least two complete sentences have been seen to reduce
//! the risk of false completions.
//!
//! The [`word_stream`] function splits the input into Unicode word boundaries.

use futures::{Stream, StreamExt};
use once_cell::sync::Lazy;
use pragmatic_segmenter::Segmenter as PragmaticSegmenter;
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;

static SEGMENTER: Lazy<PragmaticSegmenter> =
    Lazy::new(|| PragmaticSegmenter::new().expect("segmenter init"));

fn process_segment(buf: &mut String, leftover: &mut String, pending: &mut VecDeque<String>) {
    let mut segments: Vec<String> = SEGMENTER.segment(buf).map(|s| s.to_string()).collect();
    if !segments.is_empty() {
        *leftover = segments.pop().unwrap();
        for s in segments {
            pending.push_back(s);
        }
    }
    buf.clear();
}

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
                        process_segment(&mut buf, &mut leftover, &mut pending);
                    }
                    Some(Err(e)) => return Some((Err(e), (input, buf, leftover, pending))),
                    None => {
                        if !leftover.is_empty() {
                            buf.push_str(&leftover);
                            leftover.clear();
                            process_segment(&mut buf, &mut leftover, &mut pending);
                            if !leftover.is_empty() {
                                pending.push_back(std::mem::take(&mut leftover));
                            }
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

/// Stateful sentence segmenter.
///
/// This struct mirrors the logic used by [`sentence_stream`], allowing
/// synchronous segmentation without duplicating algorithms.
pub struct SentenceSegmenter {
    buf: String,
    leftover: String,
    pending: VecDeque<String>,
}

impl SentenceSegmenter {
    /// Create a new `SentenceSegmenter`.
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            leftover: String::new(),
            pending: VecDeque::new(),
        }
    }

    /// Push a text chunk and return any completed sentences.
    pub fn push_str(&mut self, chunk: &str) -> Vec<String> {
        self.buf.push_str(&self.leftover);
        self.buf.push_str(chunk);
        process_segment(&mut self.buf, &mut self.leftover, &mut self.pending);
        let mut out = Vec::new();
        while self.pending.len() > 1 {
            if let Some(sentence) = self.pending.pop_front() {
                out.push(sentence.trim().to_string());
            }
        }
        out
    }

    /// Drain any remaining sentences after all chunks are processed.
    pub fn finish(mut self) -> Vec<String> {
        if !self.leftover.is_empty() {
            self.buf.push_str(&self.leftover);
            self.leftover.clear();
            process_segment(&mut self.buf, &mut self.leftover, &mut self.pending);
            if !self.leftover.is_empty() {
                self.pending.push_back(std::mem::take(&mut self.leftover));
            }
        }
        self.pending
            .into_iter()
            .map(|s| s.trim().to_string())
            .collect()
    }
}

impl Default for SentenceSegmenter {
    fn default() -> Self {
        Self::new()
    }
}

/// Segment a block of `text` into sentences.
///
/// This uses the same logic as [`sentence_stream`].
///
/// # Examples
/// ```
/// use lingproc::segment_text_into_sentences;
///
/// let sentences = segment_text_into_sentences("Hello world. How are you?");
/// assert_eq!(sentences, vec!["Hello world.".to_string(), "How are you?".to_string()]);
/// ```
pub fn segment_text_into_sentences(text: &str) -> Vec<String> {
    let mut seg = SentenceSegmenter::new();
    let mut out = seg.push_str(text);
    out.extend(seg.finish());
    out
}
