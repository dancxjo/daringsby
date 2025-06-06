pub mod sensation;
pub mod sensor;
pub mod heartbeat;
pub mod ws;

pub use sensation::Sensation;
pub use sensor::Sensor;

pub fn placeholder() {
    println!("sensor module initialized");
}
