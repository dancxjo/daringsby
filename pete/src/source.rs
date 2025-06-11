//! Embedded Rust source files for the entire workspace.
//!
//! This module exposes the contents of every `src/*.rs` file so tests can
//! verify example code without bundling unrelated assets.
//!
//! # Examples
//! ```
//! use pete::source::{list_files, get_file};
//! let files = list_files();
//! assert!(files.contains(&"pete/src/main.rs"));
//! let main = get_file("pete/src/main.rs").unwrap();
//! assert!(main.contains("pete"));
//! ```

include!(concat!(env!("OUT_DIR"), "/sources.rs"));

/// Return the list of bundled file paths.
pub fn list_files() -> Vec<&'static str> {
    FILES.iter().map(|(n, _)| *n).collect()
}

/// Retrieve a bundled file by name.
pub fn get_file(name: &str) -> Option<&'static str> {
    FILES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}
