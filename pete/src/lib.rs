//! Pete web server library.
//!
//! This crate exposes helpers for running the Pete chatbot server and
//! interacting with a [`Psyche`](psyche::Psyche) instance.

mod ear;
mod logging;
mod mouth;
mod psyche_factory;
#[cfg(feature = "tts")]
mod tts_mouth;
mod web;

pub use ear::{ChannelEar, NoopEar};
pub use logging::init_logging;
pub use mouth::{ChannelMouth, NoopMouth};
pub use psyche_factory::{dummy_psyche, ollama_psyche};
#[cfg(feature = "tts")]
pub use tts_mouth::{CoquiTts, Tts, TtsMouth};
pub use web::{AppState, WsRequest, app, index, listen_user_input, log_ws_handler, ws_handler};
