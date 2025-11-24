//! Dylint lint crate enforcing that every module begins with an inner doc
//! comment.
//!
//! When the `dylint-driver` feature is enabled this crate exposes the
//! `module_must_have_inner_docs` lint so it can be loaded via Dylint. The lint
//! inspects source modules (`mod foo { .. }` as well as file-backed modules)
//! and emits a warning whenever the body does not start with a `//!` style
//! comment or `#![doc = "..."]` attribute placed before other inner
//! attributes. Inline examples live under `ui/` and cover inline modules,
//! file-backed modules declared with `#[path]`, and macro-generated modules to
//! demonstrate the lint’s behaviour.
//!
//! ```ignore
//! #![warn(module_must_have_inner_docs)]
//!
//! mod documented {
//!     //! Explain the module before adding code.
//!     pub fn value() {}
//! }
//! ```
//!
//! Consumers typically depend on the compiled lint crate and point `dylint`
//! (or `cargo dylint`) at the produced shared library.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod driver;

#[cfg(feature = "dylint-driver")]
pub use driver::*;
