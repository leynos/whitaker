//! Macro-aware cognitive complexity lint for Whitaker.
//!
//! The lint reports a function whose cognitive complexity exceeds a configured
//! limit while charging only nodes a person authored. Macro expansions and
//! compiler AST passes manufacture branching that no author wrote, so their
//! nodes are ignored and cannot inflate a score.
//!
//! The compiler-free scoring kernel lives in [`policy`]. A rustc driver will be
//! added behind a `dylint-driver` feature in a later milestone; the kernel
//! stays ungated so its arithmetic can be exercised without linking rustc.

pub mod policy;
