//! Helpers for webcam capture and face recognition.
//!
//! The `vision` crate is currently experimental. Future versions will allow
//! Pete to track faces and extract visual cues from a webcam feed.

pub mod face;

/// Simple debug hook.
pub fn placeholder() {
    println!("vision module initialized");
}
