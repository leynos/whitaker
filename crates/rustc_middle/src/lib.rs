#![feature(rustc_private)]
#![recursion_limit = "512"]

//! Re-exports the nightly `rustc_middle` crate for lint scaffolding.
//!
//! This crate provides access to the compiler's middle layer so template code
//! and generated lint crates can reason about MIR structures without declaring
//! unstable upstream dependencies themselves.

extern crate rustc_middle as upstream;

pub use upstream::*;
