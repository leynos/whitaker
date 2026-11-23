//! Interim behaviour record: this fixture is expected to be inverted by 2.2.12.
//!
//! RFC 0003 §9 says a function in an ordinary library source file passes even
//! when that library is compiled for a test harness. Until roadmap item 2.2.12
//! adds configured include patterns, the interim path policy selects every real
//! file, so this private helper outside any test module is reported anyway.
//!
//! The behaviour is recorded here rather than left implicit: 2.2.12 makes an
//! empty or absent `include` inert, at which point this fixture must become a
//! pass and its `.stderr` must be deleted.

#![allow(dead_code)]

fn private_library_helper() {}

/// Entry point, documented so it is not reported.
fn main() { private_library_helper(); }