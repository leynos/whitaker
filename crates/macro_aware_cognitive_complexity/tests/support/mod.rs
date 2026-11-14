//! Shared plumbing for the macro-aware cognitive-complexity integration tests.
//!
//! The parity check builds this lint crate and runs pinned Clippy and this lint
//! over one generated fixture, so these helpers own nested Cargo invocation,
//! fixture staging, and diagnostic parsing. Later feature-matrix targets reuse
//! them, and the fixture source lives in `tests/data` as data.
//!
//! The module is split by responsibility so each file stays inside the
//! repository's line budget: [`cargo`] locates the workspace and stages the
//! lint library, [`fixture`] writes and locates the pinned fixture inputs, and
//! [`run`] invokes both tools and parses their diagnostics.

pub(crate) mod cargo;
pub(crate) mod fixture;
pub(crate) mod run;
