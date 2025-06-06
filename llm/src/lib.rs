pub mod client;
pub mod model;
pub mod pool;
pub mod traits;

pub use client::OllamaClient;
pub use model::{LLMModel, LLMServer};
pub use pool::LLMClientPool;
pub use traits::{LLMAttribute, LLMCapability, LLMClient, LLMError};
