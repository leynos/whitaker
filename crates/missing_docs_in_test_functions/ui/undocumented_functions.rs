//! Undocumented functions are diagnosed with the caret on the identifier.
//!
//! Every function form that is still an `ItemKind::Fn` before expansion is in
//! scope, so `async`, `const`, and `unsafe` are all reported. The documented
//! `main` below is the control: it proves the run is not simply reporting every
//! function it sees.

#![expect(dead_code, reason = "UI test fixture; functions are analysed but never invoked")]

fn plain() {}

async fn asynchronous() {}

const fn constant() {}

unsafe fn unsafe_function() {}

/// Entry point, documented so it is not reported.
fn main() {}
