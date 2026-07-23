#![feature(rustc_private)]

//! Re-exports the nightly `rustc_lint` crate for lint scaffolding.
//!
//! The wrapper ensures generated lint crates can depend on the compiler's lint
//! infrastructure via workspace dependencies rather than linking directly to
//! unstable upstream crates.

extern crate rustc_driver;

extern crate rustc_errors;
extern crate rustc_lint as upstream;

/// Provides the compiler diagnostic decorator intended for lint implementations.
pub use rustc_errors::DiagDecorator;
pub use upstream::*;

pub mod errors {
    //! Diagnostic-construction helpers from `rustc_errors` needed by lint
    //! emission call sites (e.g. `errors::DiagDecorator`).
    pub use rustc_errors::{Diag, DiagDecorator};
}
