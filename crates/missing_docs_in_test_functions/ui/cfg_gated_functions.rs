//! `cfg` has not been evaluated when this pass runs, so what it sees is what was
//! written.
//!
//! Two functions are gated on configurations that will never hold. Both are
//! still reported, because `cfg` stripping happens after the pre-expansion
//! pass. This is a limitation record rather than a requirement — a later item
//! may revisit it — and the sibling `cfg`-gated *module* behaves differently:
//! a disabled out-of-line module is never loaded, so its functions are never
//! seen at all.

#![allow(dead_code, unexpected_cfgs)]

#[cfg(any())]
fn disabled_by_impossible_cfg() {}

#[cfg(not(test))]
fn disabled_because_this_is_a_test_harness() {}

#[cfg(any())]
mod never_loaded;

/// Entry point, documented so it is not reported.
fn main() {}