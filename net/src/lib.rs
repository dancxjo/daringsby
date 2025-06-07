//! Networking utilities for Pete's world.
//!
//! This crate collects helpers for establishing WebSocket connections and
//! building lightweight message protocols between Pete's components. It keeps
//! network concerns isolated from the higher level agents.

/// Simple test hook.
pub fn placeholder() {
    println!("net module initialized");
}

pub mod stream_bus;
