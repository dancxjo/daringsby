//! Linguistic processing utilities.
//!
//! This crate provides traits for interacting with language models, an
//! [`OllamaProvider`] implementation, and helpers for splitting LLM output into
//! sentences or words.

pub mod provider;
pub mod segment;
pub mod segmenter;
pub mod types;

pub use crate::provider::*;
pub use crate::segment::*;
pub use crate::segmenter::*;
pub use crate::types::*;
