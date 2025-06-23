//! Stateful sentence segmenter utilities.
//!
//! This module provides helpers to split static text or streaming
//! chunks into sentences. It wraps the `pragmatic_segmenter` crate
//! using the same buffering logic as `lingproc::sentence_stream`.

use futures::{Stream, StreamExt};
use pragmatic_segmenter::Segmenter as PragmaticSegmenter;
use std::collections::VecDeque;

/// Segment a block of `text` into sentences.
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
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

/// Stream sentences from a stream of text chunks.
///
/// This adapts [`lingproc::sentence_stream`] for errorless input.
pub fn stream_sentence_chunks<S>(input: S) -> impl Stream<Item = String>
where
    S: Stream<Item = String> + Unpin + Send + 'static,
{
    crate::sentence_stream(input.map(|s| Ok::<_, ()>(s))).filter_map(|r| async move { r.ok() })
}

/// A stateful sentence segmenter.
///
/// Feed chunks using [`push_str`] and retrieve completed sentences
/// when available. Call [`finish`] to flush any trailing text.
pub struct SentenceSegmenter {
    buf: String,
    leftover: String,
    pending: VecDeque<String>,
    inner: PragmaticSegmenter,
}

impl SentenceSegmenter {
    /// Create a new `SentenceSegmenter`.
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            leftover: String::new(),
            pending: VecDeque::new(),
            inner: PragmaticSegmenter::new().expect("segmenter init"),
        }
    }

    /// Push a text chunk and return any completed sentences.
    pub fn push_str(&mut self, chunk: &str) -> Vec<String> {
        self.buf.push_str(&self.leftover);
        self.buf.push_str(chunk);
        let mut segs: Vec<String> = self
            .inner
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

    /// Drain any remaining sentences after all chunks are processed.
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
