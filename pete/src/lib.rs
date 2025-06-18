//! Pete web server library.
//!
//! This crate exposes helpers for running the Pete chatbot server and interacting with a [`psyche::Psyche`] instance. It wires HTTP and WebSocket endpoints to the psyche and provides mouth/ear implementations.

mod ear;
mod face;
mod logging;
mod mouth;
mod psyche_factory;
mod sensor;
#[cfg(feature = "tts")]
mod tts_mouth;
mod web;

pub use ear::{ChannelEar, NoopEar};
pub use face::{ChannelCountenance, NoopFace};
pub use logging::init_logging;
pub use mouth::{ChannelMouth, NoopMouth};
pub use psyche_factory::{dummy_psyche, ollama_psyche};
pub use sensor::eye::EyeSensor;
#[cfg(feature = "tts")]
pub use tts_mouth::{CoquiTts, Tts, TtsMouth};
pub use web::{
    AppState, WsRequest, app, conversation_log, index, listen_user_input, log_ws_handler,
    ws_handler,
};
