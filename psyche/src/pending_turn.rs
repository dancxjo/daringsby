//! Lightweight atomic buffer for a pending user turn.
//!
//! `PendingTurn` avoids mutex contention by using an [`AtomicCell`]
//! to store an optional prompt string. It provides simple `set` and
//! `take` operations for producers and consumers.
//!
//! ```
//! use psyche::PendingTurn;
//! let buf = PendingTurn::new();
//! buf.set("hi".to_string());
//! assert_eq!(buf.take(), Some("hi".to_string()));
//! assert_eq!(buf.take(), None);
//! assert!(buf.is_empty());
//! ```
//!
//! The cell is lock-free on supported platforms and falls back to a
//! global lock otherwise.

use crossbeam_utils::atomic::AtomicCell;

/// Atomically shared pending turn buffer.
#[derive(Default)]
pub struct PendingTurn {
    inner: AtomicCell<Option<String>>,
}

impl PendingTurn {
    /// Create an empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `prompt` for later retrieval, replacing any existing value.
    pub fn set(&self, prompt: String) {
        self.inner.store(Some(prompt));
    }

    /// Take the pending prompt if present.
    pub fn take(&self) -> Option<String> {
        self.inner.take()
    }

    /// Return `true` when no turn is pending.
    pub fn is_empty(&self) -> bool {
        let cur = self.inner.take();
        let empty = cur.is_none();
        if let Some(val) = cur {
            self.inner.store(Some(val));
        }
        empty
    }
}
