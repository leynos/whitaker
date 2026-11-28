//! Dylint pre-expansion lint requiring documentation on test functions.
//!
//! The lint reports source-authored functions that carry no documentation
//! comment, but only while rustc is compiling a test harness. It runs over the
//! parsed abstract syntax tree before any macro expansion, so a function
//! decorated with `#[rstest]` or `#[given(...)]` is inspected exactly as it was
//! written rather than as its macro rewrote it.
//!
//! The compiler-free candidate-selection kernel lives in [`policy`]. The
//! rustc-backed driver sits behind the `dylint-driver` feature so the kernel's
//! truth table can be exercised, and its doctests compiled, without linking
//! rustc.

#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

pub mod policy;

#[cfg(feature = "dylint-driver")]
mod driver;

#[cfg(feature = "dylint-driver")]
pub use driver::{MISSING_DOCS_IN_TEST_FUNCTIONS, MissingDocsInTestFunctions};

#[cfg(all(test, feature = "dylint-driver"))]
#[path = "../tests/support/tests.rs"]
mod tests;
