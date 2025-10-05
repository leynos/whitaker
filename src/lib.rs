//! Core Whitaker library surfaces shared configuration and helpers for lint crates.
#![feature(rustc_private)]

pub mod config;

pub use config::{ModuleMax400LinesConfig, SharedConfig};

/// Returns a greeting for the library.
#[must_use]
pub fn greet() -> &'static str {
    "Hello from Whitaker!"
}
