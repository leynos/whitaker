#![feature(rustc_private)]

//! Re-exports the nightly `rustc_data_structures` crate for lint scaffolding.
//!
//! This proxy crate exposes the upstream compiler crate so lint templates and
//! scaffolding code can integrate with the compiler without each generated
//! project reaching into unstable internals directly.

#![expect(
    unused_extern_crates,
    reason = "Proxy crate re-exports compiler internals for Dylint shims"
)]
extern crate rustc_data_structures as upstream;
extern crate rustc_driver;

pub use upstream::*;
