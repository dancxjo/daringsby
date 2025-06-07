//! Abstractions for interacting with large language model servers.
//!
//! The `llm` crate defines a [`LLMClient`] trait along with concrete
//! implementations such as [`OllamaClient`]. Utilities are provided for routing
//! requests to multiple models and for streaming responses.

pub mod client;
pub mod model;
pub mod pool;
pub mod runner;
pub mod task;
pub mod traits;

pub use client::OllamaClient;
pub use model::{LLMModel, LLMServer};
pub use pool::LLMClientPool;
pub use pool::LLMClientPool as LinguisticScheduler; // alias for narrative terminology
pub use runner::{client_from_env, model_from_env, scheduler_from_env, run_from_env, stream_first_sentence};
pub use task::LinguisticTask;
pub use traits::{LLMAttribute, LLMCapability, LLMClient, LLMError};
