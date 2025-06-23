//! Execute dynamically parsed instructions.
//!
//! The [`InstructionExecutor`] trait represents a generic actuator that Pete can
//! invoke via a `<motor>` tag emitted by the [`Will`](crate::wits::Will)
//! component. Each implementation performs a real-world action using the given
//! attributes and body content.
//!
//! ```
//! use psyche::motorcall::{InstructionExecutor, InstructionRegistry};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! /// Motor that records each call.
//! #[derive(Default)]
//! struct RecMotor(std::sync::Mutex<Vec<(HashMap<String, String>, String)>>);
//!
//! #[async_trait]
//! impl InstructionExecutor for RecMotor {
//!     async fn execute(&self, attrs: HashMap<String, String>, content: String) {
//!         self.0.lock().unwrap().push((attrs, content));
//!     }
//! }
//!
//! # async fn doc() {
//! let mut registry = InstructionRegistry::default();
//! let motor = Arc::new(RecMotor::default());
//! registry.register("say", motor.clone());
//! registry
//!     .invoke("say", HashMap::new(), "hi".into())
//!     .await;
//! assert_eq!(motor.0.lock().unwrap().len(), 1);
//! # }
//! ```
//!
//! Implementations live in the host crate ([`pete`]) and may forward actions to
//! hardware, files, or network APIs.

/// A simple logging motor implementation.
///
/// ```
/// use psyche::motorcall::InstructionExecutor;
/// use async_trait::async_trait;
/// use std::collections::HashMap;
///
/// pub struct LoggingMotor;
///
/// #[async_trait]
/// impl InstructionExecutor for LoggingMotor {
///     async fn execute(&self, attrs: HashMap<String, String>, content: String) {
///         tracing::info!(?attrs, %content, "MOTOR fired");
///     }
/// }
/// ```
///
/// A text-to-speech motor might look like:
///
/// ```
/// use psyche::motorcall::InstructionExecutor;
/// use async_trait::async_trait;
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// #[async_trait]
/// pub trait Tts: Send + Sync {
///     async fn speak(&self, voice: Option<String>, text: String) -> Result<(), ()>;
/// }
///
/// pub struct TtsMotor {
///     pub tts: Arc<dyn Tts>,
/// }
///
/// #[async_trait]
/// impl InstructionExecutor for TtsMotor {
///     async fn execute(&self, attrs: HashMap<String, String>, content: String) {
///         let voice = attrs.get("voice").cloned();
///         if let Err(e) = self.tts.speak(voice, content).await {
///             tracing::error!(?e, "tts speak failed");
///         }
///     }
/// }
/// ```
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

#[async_trait]
pub trait InstructionExecutor: Send + Sync {
    async fn execute(&self, attrs: HashMap<String, String>, content: String);
}

#[derive(Clone, Default)]
pub struct InstructionRegistry {
    motors: HashMap<String, Arc<dyn InstructionExecutor>>,
}

impl InstructionRegistry {
    pub fn register(&mut self, name: &str, motor: Arc<dyn InstructionExecutor>) {
        self.motors.insert(name.to_string(), motor);
    }

    pub async fn invoke(&self, name: &str, attrs: HashMap<String, String>, content: String) {
        if let Some(m) = self.motors.get(name) {
            info!(target: "motor", %name, ?attrs, %content, "invoking motor");
            m.execute(attrs, content).await;
        } else {
            info!(target: "motor", %name, "motor not found");
        }
    }
}
