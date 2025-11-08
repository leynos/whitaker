#![feature(rustc_private)]

//! Re-exports the nightly `rustc_span` crate for lint scaffolding.
//!
//! Generated lint crates consume span utilities for diagnostics and reporting.
//! This wrapper forwards the entire upstream API through a workspace crate so
//! projects keep a consistent dependency surface.

#![expect(
    unused_extern_crates,
    reason = "Proxy crate re-exports compiler internals for Dylint shims"
)]
extern crate rustc_driver;
extern crate rustc_span as upstream;

pub use upstream::*;
