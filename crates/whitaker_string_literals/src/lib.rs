//! Pure analysis and rewrite construction for Rust string-literal continuations.
//!
//! This crate owns byte-oriented domain logic that can be tested without
//! loading rustc's compiler driver. The lint adapter consumes its facts and
//! rewrite plans.
#![deny(clippy::arithmetic_side_effects)]

pub mod classification;
pub mod continuation;
pub mod facts;
pub mod rewrite;
