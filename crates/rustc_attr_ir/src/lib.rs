#![feature(rustc_private)]

//! Re-exports the nightly `rustc_attr_ir` crate for lint scaffolding.
//!
//! From `nightly-2026-11-05` the HIR `Attribute` enum and its `AttributeKind`
//! live here rather than in `rustc_hir`. This proxy crate exposes the upstream
//! compiler crate so lint crates can name those types without each reaching
//! into unstable internals directly.

extern crate rustc_driver;

extern crate rustc_attr_ir as upstream;

pub use upstream::*;
