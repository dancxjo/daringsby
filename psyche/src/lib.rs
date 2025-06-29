//! Core cognitive engine powering Pete.

mod default_prompt;
mod instruction;
pub mod psyche;
pub mod sensation;
pub mod topics;
pub mod util;
mod voice;

/// Traits are organized in submodules and then re-exported for crate users.
/// Keeping this module public enables paths like `psyche::traits::Ear` while
/// also flattening common traits at the crate root below.
pub mod traits {
    pub mod buffered_wit;
    pub mod doer;
    pub mod ear;
    pub mod motor;
    pub mod mouth;
    pub mod observer;
    pub mod sensor;
    pub mod tts;
    pub mod wit;

    pub use buffered_wit::BufferedWit;
    pub use doer::Doer;
    pub use ear::Ear;
    pub use motor::{Motor, NoopMotor};
    pub use mouth::Mouth;
    pub use observer::SensationObserver;
    pub use sensor::Sensor;
    pub use tts::{Tts, TtsStream};
    pub use wit::{ErasedWit, Wit, WitAdapter};
}

pub mod wit;
pub mod wits {
    pub mod combobulator;
    pub mod entity_wit;
    pub mod episode_wit;
    pub mod face_memory_wit;
    pub mod fond_du_coeur;
    pub mod heart_wit;
    pub mod identity_wit;
    pub mod memory;
    pub mod memory_wit;
    pub mod moment_wit;
    pub mod quick;
    pub mod situation_wit;
    pub mod vision_wit;
    pub mod will;

    pub use combobulator::Combobulator;
    pub use entity_wit::EntityWit;
    pub use episode_wit::EpisodeWit;
    pub use face_memory_wit::FaceMemoryWit;
    pub use fond_du_coeur::FondDuCoeur;
    pub use heart_wit::HeartWit;
    pub use identity_wit::IdentityWit;
    pub use memory::{BasicMemory, GraphStore, Memory, Neo4jClient, NoopMemory, QdrantClient};
    pub use memory_wit::MemoryWit;
    pub use moment_wit::MomentWit;
    pub use quick::Quick;
    pub use situation_wit::SituationWit;
    pub use vision_wit::VisionWit;
    pub use will::Will;
}

mod and_mouth;

mod debug;

pub mod ling;
/// Model types are public for introspection but also re-exported below so
/// callers can simply `use psyche::{Experience, Impression}`.
pub mod model;
pub mod motor_call;
mod plain_mouth;
/// The `prompt` module is kept public so callers may use `psyche::prompt::*`.
/// Key prompt types are also re-exported at the crate root for convenience.
pub mod prompt;
mod task_group;
pub use task_group::TaskGroup;
pub mod sensors {
    #[cfg(feature = "face")]
    pub mod face;
    #[cfg(feature = "face")]
    pub use face::{DummyDetector, FaceDetector, FaceInfo, FaceSensor};
}
mod pending_turn;
mod trim_mouth;
mod types;

pub use and_mouth::AndMouth;
pub use debug::{DebugHandle, DebugInfo, debug_enabled, disable_debug, enable_debug};
pub use default_prompt::DEFAULT_SYSTEM_PROMPT;
pub use instruction::{HostInstruction, parse_instructions};
pub use model::{Experience, Impression, Stimulus};
pub use pending_turn::PendingTurn;
pub use plain_mouth::PlainMouth;
pub use prompt::{CombobulatorPrompt, ContextualPrompt, PromptFragment, VoicePrompt, WillPrompt};
pub use topics::{Topic, TopicBus, TopicMessage};
pub use trim_mouth::TrimMouth;
pub use types::{Decision, GeoLoc, Heartbeat, ImageData, ObjectInfo};

pub use ling::{Feeling, PromptBuilder};
pub use psyche::extract_tag as test_extract_tag;
pub use psyche::{Conversation, Psyche};
pub use sensation::{Event, Instant, Sensation, WitReport};
#[cfg(feature = "face")]
pub use sensors::{DummyDetector, FaceDetector, FaceInfo, FaceSensor};
pub use traits::{
    BufferedWit, Doer, Ear, ErasedWit, Motor, Mouth, NoopMotor, SensationObserver, Sensor, Tts,
    TtsStream, Wit, WitAdapter,
};
pub use voice::{Voice, extract_emojis};
pub use wits::{
    BasicMemory, Combobulator, EntityWit, EpisodeWit, FaceMemoryWit, FondDuCoeur, GraphStore,
    HeartWit, IdentityWit, Memory, MemoryWit, Neo4jClient, NoopMemory, QdrantClient, VisionWit,
    Will,
};
