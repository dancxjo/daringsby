//! Lightweight synchronized buffer for a pending user turn.
//!
//! `PendingTurn` stores an optional prompt string and notifies waiters when a
//! producer queues a turn. It provides simple `set` and `take` operations for
//! producers and consumers.
//!
//! ```
//! use psyche::PendingTurn;
//! let buf = PendingTurn::new();
//! buf.set("hi".to_string());
//! assert_eq!(buf.take(), Some("hi".to_string()));
//! assert_eq!(buf.take(), None);
//! assert!(buf.is_empty());
//! ```
use std::sync::Mutex;
use tokio::sync::Notify;

/// Shared pending turn buffer.
pub struct PendingTurn {
    inner: Mutex<Option<String>>,
    notify: Notify,
}

impl Default for PendingTurn {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
            notify: Notify::new(),
        }
    }
}

impl PendingTurn {
    /// Create an empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `prompt` for later retrieval, replacing any existing value.
    pub fn set(&self, prompt: String) {
        *self.inner.lock().unwrap() = Some(prompt);
        self.notify.notify_one();
    }

    /// Take the pending prompt if present.
    pub fn take(&self) -> Option<String> {
        self.inner.lock().unwrap().take()
    }

    /// Return `true` when no turn is pending.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_none()
    }

    /// Wait until a producer queues a turn.
    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}
