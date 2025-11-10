#![feature(rustc_private)]
#![recursion_limit = "512"]

//! Re-exports the nightly `rustc_session` crate for lint scaffolding.
//!
//! Lint projects rely on the compiler session for configuration data and
//! diagnostic emission. This proxy crate exposes the upstream session API so
//! templates can depend on a stable workspace path.

extern crate rustc_driver;

extern crate rustc_session as upstream;

pub use upstream::*;
