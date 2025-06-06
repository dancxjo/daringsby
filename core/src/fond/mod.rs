use async_trait::async_trait;
use sensor::Sensation;

use crate::genie::{Genie, GenieError};

/// Simple [`Genie`] that concatenates all sensations into an identity paragraph.
pub struct FondDuCoeur {
    identity: String,
    queue: Vec<Sensation>,
}

impl FondDuCoeur {
    /// Create an empty instance.
    pub fn new() -> Self {
        Self {
            identity: String::new(),
            queue: Vec::new(),
        }
    }

    /// Return the current identity summary.
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

#[async_trait]
impl Genie for FondDuCoeur {
    /// Queue a sensation to be folded into the identity summary.
    async fn feel(&mut self, sensation: Sensation) {
        self.queue.push(sensation);
    }

    /// Summarize all queued sensations into a single string.
    async fn consult(&mut self) -> Result<String, GenieError> {
        if !self.queue.is_empty() {
            for s in self.queue.drain(..) {
                if !self.identity.is_empty() {
                    self.identity.push(' ');
                }
                self.identity.push_str(&s.how);
            }
        }
        if self.identity.is_empty() {
            Err(GenieError::Empty)
        } else {
            Ok(self.identity.clone())
        }
    }
}
