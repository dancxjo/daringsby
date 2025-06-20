use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{collections::HashMap, collections::VecDeque, sync::Arc};
use tokio::sync::Mutex;

use crate::Sensation;

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
