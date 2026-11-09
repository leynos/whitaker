//! Macro-aware cognitive complexity lint for Whitaker.
//!
//! The lint reports a function whose cognitive complexity exceeds a configured
//! limit while charging only nodes a person authored. Macro expansions and
//! compiler AST passes manufacture branching that no author wrote, so their
//! nodes are ignored and cannot inflate a score.
//!
//! The compiler-free scoring kernel lives in [`policy`]. The rustc-backed
//! driver sits behind the `dylint-driver` feature so the kernel's arithmetic
//! can be exercised without linking rustc.

#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

pub mod policy;

#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(feature = "dylint-driver")]
pub use driver::{MACRO_AWARE_COGNITIVE_COMPLEXITY, MacroAwareCognitiveComplexity};
