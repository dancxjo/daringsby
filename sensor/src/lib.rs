//! Input devices for Pete's world.
//!
//! Sensors produce [`Sensation`] structs and stream them through an async
//! channel to the rest of the system.

pub mod heartbeat;
pub mod sensation;
pub mod sensor;
pub mod ws;

pub use sensation::Sensation;
pub use sensor::Sensor;

/// Convenience helper to prove the crate links.
pub fn placeholder() {
    println!("sensor module initialized");
}
