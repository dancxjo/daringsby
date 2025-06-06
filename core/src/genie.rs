use async_trait::async_trait;
use sensor::Sensation;
use thiserror::Error;

/// Errors that can occur when consulting a [`Genie`].
#[derive(Debug, Error)]
pub enum GenieError {
    #[error("no reflection available")]
    Empty,
}

/// Abstraction over modules that can absorb [`Sensation`]s and produce a summary.
#[async_trait]
pub trait Genie {
    async fn feel(&mut self, sensation: Sensation);
    async fn consult(&mut self) -> Result<String, GenieError>;
}
