//! Bounded verification harnesses for AST feature extraction.
//!
//! Stage F adds concrete Kani harnesses over parser-independent
//! [`super::NormalisedTree`] values. The module exists from Stage A so the
//! `#[cfg(kani)]` boundary is stable and tooling can resolve the module graph.
