//! Core cognitive engine powering Pete.

pub mod psyche;
pub mod sensation;
mod voice;

pub mod traits {
    pub mod ear;
    pub mod mouth;
    pub mod observer;
    pub mod wit;

    pub use ear::Ear;
    pub use mouth::Mouth;
    pub use observer::SensationObserver;
    pub use wit::{ErasedWit, Summarizer, Wit, WitAdapter};
}

pub mod wit;
pub mod wits {
    pub mod combobulator;
    pub mod combobulator_wit;
    pub mod fond_du_coeur;
    pub mod fond_du_coeur_wit;
    pub mod heart_wit;
    pub mod memory;
    pub mod memory_wit;
    pub mod vision_wit;
    pub mod will;
    pub mod will_wit;

    pub use combobulator::Combobulator;
    pub use combobulator_wit::CombobulatorWit;
    pub use fond_du_coeur::FondDuCoeur;
    pub use fond_du_coeur_wit::FondDuCoeurWit;
    pub use heart_wit::HeartWit;
    pub use memory::{BasicMemory, GraphStore, Memory, Neo4jClient, NoopMemory, QdrantClient};
    pub use memory_wit::MemoryWit;
    pub use vision_wit::VisionWit;
    pub use will::Will;
    pub use will_wit::WillWit;
}

mod and_mouth;

mod debug;
mod impression;
pub mod ling;
pub mod model;
mod motor;
pub mod motorcall;
mod plain_mouth;
mod prehension;
mod prompt;
mod sensor;
mod trim_mouth;
mod types;

pub use and_mouth::AndMouth;
pub use debug::{DebugHandle, DebugInfo, debug_enabled, disable_debug, enable_debug};
pub use impression::Impression;
pub use model::{Experience, Impression as NewImpression, Stimulus};
pub use motor::{Motor, NoopMotor};
pub use plain_mouth::PlainMouth;
pub use prehension::Prehension;
pub use prompt::{CombobulatorPrompt, PromptBuilder, VoicePrompt, WillPrompt};
pub use psyche::DEFAULT_SYSTEM_PROMPT;
pub use sensor::Sensor;
pub use trim_mouth::TrimMouth;
pub use types::ImageData;

pub use ling::{Feeling, Ling};
pub use psyche::{Conversation, Psyche};
pub use sensation::{Event, Sensation, WitReport};
pub use traits::{Ear, ErasedWit, Mouth, SensationObserver, Summarizer, Wit, WitAdapter};
pub use voice::{Voice, extract_emojis};
pub use wits::{
    BasicMemory, CombobulatorWit, FondDuCoeur, FondDuCoeurWit, GraphStore, HeartWit, Memory,
    MemoryWit, Neo4jClient, NoopMemory, QdrantClient, VisionWit, Will, WillWit,
};
