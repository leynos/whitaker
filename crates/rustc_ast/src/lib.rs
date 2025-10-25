#![feature(rustc_private)]
#![recursion_limit = "512"]

//! Re-exports the nightly `rustc_ast` crate for lint scaffolding.
//!
//! This proxy crate exposes the upstream compiler crate so lint templates and
//! scaffolding code can integrate with the compiler without each generated
//! project reaching into unstable internals directly.

extern crate rustc_ast as upstream;

pub use upstream::*;
