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

fn shared_segmenter() -> &'static PragmaticSegmenter {
    &SEGMENTER
}

/// Stateful sentence segmenter usable for both streaming and buffered text.
///
/// Feed text chunks with [`push_str`] and retrieve completed sentences. Any
/// trailing text can be flushed using [`finish`]. All segmentation goes through
/// the same [`pragmatic_segmenter`] instance as [`sentence_stream`].
pub struct SentenceSegmenter {
    buf: String,
    leftover: String,
    pending: VecDeque<String>,
}

impl SentenceSegmenter {
    /// Create a new `SentenceSegmenter`.
    pub fn new() -> Self {
        Self { buf: String::new(), leftover: String::new(), pending: VecDeque::new() }
    }

    /// Push a text chunk and return any fully segmented sentences.
    pub fn push_str(&mut self, chunk: &str) -> Vec<String> {
        self.buf.push_str(&self.leftover);
        self.buf.push_str(chunk);
        let mut segs: Vec<String> = shared_segmenter()
            .segment(&self.buf)
            .map(|s| s.to_string())
            .collect();
        if !segs.is_empty() {
            self.leftover = segs.pop().unwrap();
            for s in segs {
                self.pending.push_back(s);
            }
        }
        self.buf.clear();
        let mut out = Vec::new();
        while self.pending.len() > 1 {
            if let Some(sentence) = self.pending.pop_front() {
                out.push(sentence.trim().to_string());
            }
        }
        out
    }

    /// Finish segmentation and return any remaining sentences.
    pub fn finish(mut self) -> Vec<String> {
        if !self.leftover.is_empty() {
            self.pending.push_back(self.leftover);
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

/// Segment a block of `text` into sentences using the same logic as
/// [`sentence_stream`].
///
/// # Examples
/// ```
/// use lingproc::segment_text_into_sentences;
/// let s = "Hello world. How are you?";
/// assert_eq!(
///     segment_text_into_sentences(s),
///     vec!["Hello world.".to_string(), "How are you?".to_string()]
/// );
/// ```
pub fn segment_text_into_sentences(text: &str) -> Vec<String> {
    let mut seg = SentenceSegmenter::new();
    let mut out = seg.push_str(text);
    out.extend(seg.finish());
    out
}

/// Adapt [`sentence_stream`] for infallible input streams.
pub fn stream_sentence_chunks<S>(input: S) -> impl Stream<Item = String>
where
    S: Stream<Item = String> + Unpin + Send + 'static,
{
    sentence_stream(input.map(Ok::<_, ()>)).filter_map(|r| async move { r.ok() })
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
///         "David E. Sanger covers the Trump administration and a range of national security issues."
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
    let segmenter = SentenceSegmenter::new();
    let pending: VecDeque<String> = VecDeque::new();

    let stream = unfold(
        (input, segmenter, pending),
        |(mut input, mut segmenter, mut pending)| async move {
            loop {
                if let Some(next) = pending.pop_front() {
                    return Some((Ok(next), (input, segmenter, pending)));
                }
                match input.next().await {
                    Some(Ok(chunk)) => {
                        for s in segmenter.push_str(&chunk) {
                            pending.push_back(s);
                        }
                    }
                    Some(Err(e)) => return Some((Err(e), (input, segmenter, pending))),
                    None => {
                        let remaining = segmenter.finish();
                        for s in remaining {
                            pending.push_back(s);
                        }
                        segmenter = SentenceSegmenter::new();
                        if let Some(s) = pending.pop_front() {
                            return Some((Ok(s), (input, segmenter, pending)));
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
