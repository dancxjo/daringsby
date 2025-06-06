use async_trait::async_trait;
use sensor::Sensation;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenieError {
    #[error("no reflection available")] 
    Empty,
}

#[async_trait]
pub trait Genie {
    async fn feel(&mut self, sensation: Sensation);
    async fn consult(&mut self) -> Result<String, GenieError>;
}
