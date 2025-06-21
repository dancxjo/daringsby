//! Minimal cognitive kernel.

pub mod memory;
pub mod psyche;
pub mod types;
pub mod wit;

pub use memory::{Memory, NoopMemory};
pub use psyche::Psyche;
pub use types::{Experience, Impression, Stimulus};
pub use wit::Wit;
