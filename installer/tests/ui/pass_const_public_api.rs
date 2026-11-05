//! Compile-pass guard for the installer's public const API.
//!
//! Each item below is evaluated at compile time, so removing `const` from the
//! function it calls turns this fixture into a compile error and fails the
//! trybuild case.
//!
//! Two public `const` functions are deliberately absent.
//! `Cli::install_args` returns a borrow of `clap`-derived state, which cannot
//! be built in a constant. `WorkspacePlan::updating` returns a type whose
//! `Option<SuiteRef>` field gives the value a destructor, and Rust forbids
//! dropping such a value during constant evaluation, so no external caller can
//! exercise its `const`-ness today.

use whitaker_installer::{clippy_snapshot::manifest_contents, deps::DylintToolStatus};

const CLIPPY_SNAPSHOT: &str = manifest_contents();
const TOOLS_READY: DylintToolStatus = DylintToolStatus {
    cargo_dylint: true,
    dylint_link: true,
};
const ALL_INSTALLED: bool = TOOLS_READY.all_installed();
const _: () = assert!(ALL_INSTALLED && !CLIPPY_SNAPSHOT.is_empty());

fn main() {}