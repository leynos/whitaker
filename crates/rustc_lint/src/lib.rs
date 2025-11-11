#![feature(rustc_private)]

//! Re-exports the nightly `rustc_lint` crate for lint scaffolding.
//!
//! The wrapper ensures generated lint crates can depend on the compiler's lint
//! infrastructure via workspace dependencies rather than linking directly to
//! unstable upstream crates.

extern crate rustc_driver;

extern crate rustc_lint as upstream;

pub use upstream::*;
