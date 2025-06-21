use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use tokio::sync::Mutex;

use crate::Sensation;

/// Global filter controlling per-Wit debug output.
pub static DEBUG_FILTER: Lazy<Arc<Mutex<HashSet<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::new())));

/// Check if debug output is enabled for `label`.
pub async fn debug_enabled(label: &str) -> bool {
    DEBUG_FILTER.lock().await.contains(label)
}

/// Enable debug output for `label`.
pub async fn enable_debug(label: &str) {
    DEBUG_FILTER.lock().await.insert(label.to_string());
}

/// Disable debug output for `label`.
pub async fn disable_debug(label: &str) {
    DEBUG_FILTER.lock().await.remove(label);
}

/// Snapshot of internal Psyche state for debugging.
#[derive(Serialize)]
pub struct DebugInfo {
    /// Number of queued sensations.
    pub buffer_len: usize,
    /// Names of registered wits.
    pub active_wits: Vec<String>,
    /// Last tick time for each wit.
    pub last_ticks: HashMap<String, DateTime<Utc>>,
}

/// Handle providing read-only access to debug information.
#[derive(Clone)]
pub struct DebugHandle {
    pub(crate) buffer: Arc<Mutex<VecDeque<Sensation>>>,
    pub(crate) ticks: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    pub(crate) wits: Vec<String>,
}

impl DebugHandle {
    /// Gather the current [`DebugInfo`] snapshot.
    pub async fn snapshot(&self) -> DebugInfo {
        let buffer_len = self.buffer.lock().await.len();
        let last_ticks = self.ticks.lock().await.clone();
        DebugInfo {
            buffer_len,
            active_wits: self.wits.clone(),
            last_ticks,
        }
    }
}
