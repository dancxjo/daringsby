//! Pete web server library.
//!
//! This crate exposes helpers for running the Pete chatbot server and interacting with a [`psyche::Psyche`] instance. It wires HTTP and WebSocket endpoints to the psyche and provides mouth/ear implementations.

mod ear;
mod event_bus;
mod logging;
mod motor;
mod mouth;
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
#[cfg(feature = "face")]
pub use psyche::FaceSensor;
pub use psyche_factory::{dummy_psyche, ollama_psyche};
pub use sensor::NoopSensor;
#[cfg(feature = "eye")]
pub use sensor::eye::EyeSensor;
#[cfg(feature = "geo")]
pub use sensor::geo::GeoSensor;
pub use sensor::heartbeat::HeartbeatSensor;
pub use simulator::Simulator;
#[cfg(feature = "tts")]
pub use tts_mouth::{CoquiTts, Tts, TtsMouth, TtsStream};
pub use web::{
    AppState, WsRequest, app, conversation_log, index, listen_user_input, log_ws_handler,
    psyche_debug, toggle_wit_debug, wit_debug_page, ws_handler,
};
