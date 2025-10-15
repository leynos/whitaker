#![feature(rustc_private)]
#![doc = "Re-exports the compiler crate from the nightly toolchain for lint scaffolding."]

extern crate rustc_span as upstream;

pub use upstream::*;
