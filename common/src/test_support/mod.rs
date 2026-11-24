//! Shared test helpers for Whitaker crates.
//!
//! The helpers in this module are intended for use from unit and integration
//! tests so repeated boilerplate can live in one place.
//!
//! Tests do not change the process environment. Inject the value where the
//! code under test reads it, pass it to a spawned process with
//! `Command::env`, or re-run the test in a child process with
//! [`run_in_child`].
//!
//! ## Available helpers
//!
//! - [`child_process`]: Re-runs a test in a child process whose environment is set at spawn
//!   time, for variables a whole process must carry.
//! - [`fixtures`]: Copies UI fixtures (source files, `.stderr` expectations and support
//!   directories) into isolated workspaces for dylint UI harnesses.
//! - [`decomposition`]: Reusable decomposition-advice fixtures for unit and behaviour tests.
//! - [`ui`]: Discovers fixtures, prepares isolated workspaces, and runs dylint UI tests with
//!   consistent panic handling.

pub mod child_process;
pub mod decomposition;
pub mod fixtures;
pub mod ui;

pub use child_process::{ChildRole, child_test_path, run_in_child};
pub use fixtures::{copy_directory, copy_fixture};
pub use ui::{
    FixtureEnvironment,
    discover_fixtures,
    prepare_fixture,
    read_directory_config,
    read_fixture_config,
    resolve_fixture_config,
    run_fixtures_with,
    run_test_runner,
};
