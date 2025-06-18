//! Core cognitive engine powering Pete.

pub mod psyche;
pub mod sensation;

pub mod traits {
    pub mod countenance;
    pub mod ear;
    pub mod mouth;
    pub mod wit;

    pub use countenance::{Countenance, NoopCountenance};
    pub use ear::Ear;
    pub use mouth::Mouth;
    pub use wit::{ErasedWit, Summarizer, Wit, WitAdapter};
}

pub mod wit;
pub mod wits {
    pub mod heart;
    pub mod memory;
    pub mod vision_wit;
    pub mod will;

    pub use heart::Heart;
    pub use memory::{BasicMemory, GraphStore, Memory, Neo4jClient, NoopMemory, QdrantClient};
    pub use vision_wit::VisionWit;
    pub use will::Will;
}

mod and_mouth;
mod emoji_mouth;
mod impression;
pub mod ling;
mod motor;
mod plain_mouth;
mod prehension;
mod prompt;
mod sensor;
mod trim_mouth;
mod types;

pub use and_mouth::AndMouth;
pub use emoji_mouth::EmojiMouth;
pub use impression::Impression;
pub use motor::{Motor, NoopMotor};
pub use plain_mouth::PlainMouth;
pub use prehension::Prehension;
pub use prompt::{HeartPrompt, PromptBuilder, VoicePrompt, WillPrompt};
pub use psyche::DEFAULT_SYSTEM_PROMPT;
pub use sensor::Sensor;
pub use trim_mouth::TrimMouth;
pub use types::ImageData;

pub use psyche::{Conversation, Psyche};
pub use sensation::{Event, Sensation, WitReport};
pub use traits::{
    Countenance, Ear, ErasedWit, Mouth, NoopCountenance, Summarizer, Wit, WitAdapter,
};
pub use wits::{
    BasicMemory, GraphStore, Heart, Memory, Neo4jClient, NoopMemory, QdrantClient, VisionWit, Will,
};
