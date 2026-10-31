//! Exercise configuration loading with isolated files and injected environments.
//!
//! The scenarios are grouped by concern: file discovery and legacy
//! compatibility, source precedence, and validation. Shared fixtures live in
//! [`support`].

#[path = "configuration/support.rs"]
mod support;

#[path = "configuration/discovery.rs"]
mod discovery;

#[path = "configuration/precedence.rs"]
mod precedence;

#[path = "configuration/validation.rs"]
mod validation;
