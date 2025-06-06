pub mod client;
pub mod model;
pub mod pool;
pub mod runner;
pub mod traits;

pub use client::OllamaClient;
pub use model::{LLMModel, LLMServer};
pub use pool::LLMClientPool;
pub use runner::{client_from_env, model_from_env, run_from_env, stream_first_sentence};
pub use traits::{LLMAttribute, LLMCapability, LLMClient, LLMError};
