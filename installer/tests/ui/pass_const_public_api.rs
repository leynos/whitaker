//! Compile-pass guard for the installer's public const API.
//!
//! Each item below is evaluated at compile time, so removing `const` from the
//! function it calls turns this fixture into a compile error and fails the
//! trybuild case.

use whitaker_installer::{
    cli::{Cli, InstallArgs},
    clippy_snapshot::manifest_contents,
    deps::DylintToolStatus,
    workspace::WorkspacePlan,
};

const CLIPPY_SNAPSHOT: &str = manifest_contents();
const TOOLS_READY: DylintToolStatus = DylintToolStatus {
    cargo_dylint: true,
    dylint_link: true,
};
const ALL_INSTALLED: bool = TOOLS_READY.all_installed();
const _: () = assert!(ALL_INSTALLED && !CLIPPY_SNAPSHOT.is_empty());

/// Forward to `Cli::install_args` so this body is const-checked.
///
/// The body compiles only while `Cli::install_args` is a `const fn`, even
/// though the forwarding function is never called in a constant context.
const fn install_args(cli: &Cli) -> &InstallArgs {
    cli.install_args()
}

/// Bind a constant `WorkspacePlan` so the call site is const-evaluated.
const UPDATING_PLAN: WorkspacePlan = WorkspacePlan::updating(true);

fn main() {
    let _: fn(&Cli) -> &InstallArgs = install_args;
    assert!(UPDATING_PLAN.should_update);
    assert!(UPDATING_PLAN.suite_ref.is_none());
}
