//! Unit and UI test wiring for the `missing_docs_in_test_functions` crate.
//!
//! Lives under `tests/support/` and is pulled into the library with an
//! explicit `#[path]` attribute, matching the other Whitaker lint crates. The
//! behavioural scenarios and the UI fixtures are named here so one gate runs
//! them all; the pure policy tests are declared beside the modules they cover.

#[path = "behaviour.rs"]
mod behaviour;

#[path = "ui.rs"]
mod ui;
