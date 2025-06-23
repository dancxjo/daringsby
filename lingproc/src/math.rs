//! Mathematical utilities for embeddings and vector operations.
//!
//! This module currently re-exports [`cosine_similarity`] from the `common` crate
//! so other crates can continue using `lingproc::math` as before.

pub use common::cosine_similarity;
