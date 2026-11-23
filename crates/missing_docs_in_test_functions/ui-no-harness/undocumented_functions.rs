//! The same undocumented sources as `ui/undocumented_functions.rs`, compiled
//! without `--test`.
//!
//! Nothing here is reported, and the absence of diagnostics is the point: it
//! proves the harness gate, not the documentation gate, is what keeps ordinary
//! library and binary targets quiet. The lint is denied, so a diagnostic would
//! fail the compilation rather than pass unnoticed.

#![allow(dead_code)]

fn plain() {}

async fn asynchronous() {}

const fn constant() {}

unsafe fn unsafe_function() {}

fn main() {}