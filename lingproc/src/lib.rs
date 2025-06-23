//! Linguistic processing utilities.
//!
//! This crate provides traits for interacting with language models, an
//! [`OllamaProvider`] implementation, and helpers for splitting LLM output into
//! sentences or words.

pub mod math;
pub mod provider;
pub mod segment;
pub mod types;

pub use crate::math::*;
pub use crate::provider::*;
pub use crate::segment::*;
pub use crate::types::*;
