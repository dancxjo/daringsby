//! # Pete — An Embodied Consciousness Host
//!
//! This crate provides the server and runtime harness for Pete, an experimental
//! embodied artificial consciousness.
//!
//! It connects Pete’s [`psyche::Psyche`] — the cognitive core — to a real-world
//! environment via sensors, actuators, and multimodal communication channels such
//! as audio, video, geolocation, and WebSockets.
//!
//! ## Responsibilities
//!
//! - Hosts and runs the [`Psyche`] instance
//! - Provides `Mouth` and `Ear` implementations for speaking and listening
//! - Bridges between sensor input (camera, mic, GPS, etc.) and Pete’s cognitive
//!   Wits
//! - Serves WebSocket endpoints for live frontend interaction
//! - Routes textual, audio, and emotional output back to clients
//! - Launches background processing (e.g., speech recognition, TTS, face
//!   detection)
//!
//! ## Components
//!
//! - [`Body`]: Shared state container for the live app
//! - [`main.rs`]: Pete’s entry point and lifecycle wiring
//! - [`psyche_factory.rs`]: Assembles the cognitive architecture (Wits, Topics,
//!   Memory)
//! - [`tts_mouth.rs`]: TTS backend for generating speech (e.g., via Coqui)
//! - [`voice.rs`]: The inner voice agent that turns intention into words
//!
//! Pete is not just a chatbot. It is a cognitive agent with evolving memory,
//! emotion, and embodied presence. This crate provides the scaffolding and
//! external limbs through which that mind interfaces with the world.

mod ear;
mod event_bus;
mod logging;
mod motor;
mod mouth;
mod ollama;
mod psyche_factory;
mod sensor;
mod simulator;
#[cfg(feature = "tts")]
mod tts_mouth;
mod web;

#[cfg(feature = "ear")]
pub use ear::ChannelEar;
pub use ear::NoopEar;
pub use event_bus::EventBus;
pub use logging::init_logging;
pub use motor::LoggingMotor;
pub use mouth::{ChannelMouth, NoopMouth};
pub use ollama::ollama_provider_from_args;
#[cfg(feature = "face")]
pub use psyche::FaceSensor;
#[cfg(feature = "tts")]
pub use psyche::traits::{Tts, TtsStream};
pub use psyche_factory::{dummy_psyche, ollama_psyche};
pub use sensor::NoopSensor;
#[cfg(feature = "eye")]
pub use sensor::eye::EyeSensor;
#[cfg(feature = "geo")]
pub use sensor::geo::GeoSensor;
pub use sensor::heartbeat::HeartbeatSensor;
pub use simulator::Simulator;
#[cfg(feature = "tts")]
pub use tts_mouth::{CoquiTts, TtsMouth};
pub use web::{
    Body, WsRequest, app, conversation_log, index, listen_user_input, log_ws_handler, psyche_debug,
    toggle_wit_debug, wit_debug_page, ws_handler,
};
