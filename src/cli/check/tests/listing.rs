//! The `-v` coverage listing through the real command boundary.

use std::ffi::OsString;

use camino::Utf8Path;
use ortho_config::NoOpLocalizer;

use super::ScriptedPort;
use crate::{
    cli::check::{
        CargoOptions,
        CheckAdapters,
        CheckRequest,
        Outcome,
        exit::CheckOutcome,
        run_check_command,
    },
    diagnostics::DiagnosticSettings,
    ports::{
        PackageFacts,
        PackageQueryError,
        WorkspaceFacts,
        WorkspaceQuery,
        WorkspaceQueryRequest,
    },
};

/// A query that answers with one workspace member.
struct FixedWorkspace;

impl WorkspaceQuery for FixedWorkspace {
    /// Report the same facts for every request.
    fn workspace_facts(
        &self,
        _request: &WorkspaceQueryRequest<'_>,
    ) -> Result<WorkspaceFacts, PackageQueryError> {
        let package = PackageFacts {
            name: "member_b".to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: "/work/app/member_b/Cargo.toml".into(),
        };
        Ok(WorkspaceFacts {
            workspace_root: "/work/app".into(),
            members: vec![package.clone()],
            default_members: vec![package],
        })
    }
}

/// Run `check` with `settings` and a fixed workspace, returning stderr.
fn run_with(settings: DiagnosticSettings, cargo: CargoOptions) -> (Outcome, String) {
    let port = ScriptedPort::answering(Ok(()));
    let arguments = crate::cli::check::CheckArgs {
        no_install: false,
        cargo,
        cargo_args: Vec::new(),
    };
    let mut stderr = Vec::new();
    let outcome = run_check_command(
        CheckRequest::new(&arguments, settings, Some(Utf8Path::new("/work/app"))),
        &NoOpLocalizer::new(),
        CheckAdapters {
            check: &port,
            packages: &FixedWorkspace,
        },
        &mut stderr,
    );
    (outcome, String::from_utf8(stderr).expect("stderr is UTF-8"))
}

/// Show every selected package before the child runs.
#[test]
fn a_verbose_check_lists_the_packages_it_will_check() {
    let (outcome, stderr) = run_with(
        DiagnosticSettings {
            quiet: false,
            verbosity: 1,
        },
        CargoOptions {
            workspace: true,
            ..CargoOptions::default()
        },
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "the listing must not change the run's outcome"
    );
    assert!(
        stderr.contains("    Checking member_b v0.1.0 (/work/app/member_b)"),
        "the listing must use Cargo's progress-line shape, got:\n{stderr}"
    );
}

/// Say nothing at the default verbosity.
#[test]
fn a_quiet_check_prints_no_listing() {
    let (outcome, stderr) = run_with(
        DiagnosticSettings::default(),
        CargoOptions {
            workspace: true,
            ..CargoOptions::default()
        },
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "the run must still complete"
    );
    assert!(
        !stderr.contains("Checking"),
        "the default verbosity must not list packages, got:\n{stderr}"
    );
}

/// Let `-q` win even when `-v` is also present.
#[test]
fn quiet_suppresses_the_listing() {
    let (outcome, stderr) = run_with(
        DiagnosticSettings {
            quiet: true,
            verbosity: 1,
        },
        CargoOptions {
            workspace: true,
            ..CargoOptions::default()
        },
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "the run must still complete"
    );
    assert!(
        !stderr.contains("Checking"),
        "`-q` must suppress the listing, got:\n{stderr}"
    );
}

/// Name the deprecated alias before the listing, on stderr.
#[test]
fn the_deprecated_alias_is_announced() {
    let (outcome, stderr) = run_with(
        DiagnosticSettings::default(),
        CargoOptions {
            all: true,
            ..CargoOptions::default()
        },
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "the alias must not change the run's outcome"
    );
    assert!(
        stderr.contains("warning: --all is deprecated in favour of --workspace"),
        "the deprecated alias must be announced, got:\n{stderr}"
    );
}

/// Keep the notice out of a quiet invocation.
#[test]
fn quiet_suppresses_the_deprecated_alias_notice() {
    let (_, stderr) = run_with(
        DiagnosticSettings {
            quiet: true,
            verbosity: 0,
        },
        CargoOptions {
            all: true,
            ..CargoOptions::default()
        },
    );

    assert!(
        !stderr.contains("deprecated"),
        "`-q` must suppress the notice, got:\n{stderr}"
    );
}

/// Leave the forwarded arguments unchanged when the listing is on.
#[test]
fn the_listing_does_not_change_the_forwarded_arguments() {
    let port = ScriptedPort::answering(Ok(()));
    let arguments = crate::cli::check::CheckArgs {
        no_install: false,
        cargo: CargoOptions {
            packages: vec!["member_a".to_owned()],
            ..CargoOptions::default()
        },
        cargo_args: vec!["-q".into()],
    };
    let mut stderr = Vec::new();
    let outcome = run_check_command(
        CheckRequest::new(
            &arguments,
            DiagnosticSettings {
                quiet: false,
                verbosity: 1,
            },
            Some(Utf8Path::new("/work/app")),
        ),
        &NoOpLocalizer::new(),
        CheckAdapters {
            check: &port,
            packages: &FixedWorkspace,
        },
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "the run must complete"
    );
    assert_eq!(
        port.executed
            .lock()
            .expect("the test owns this lock")
            .as_slice(),
        [vec![
            OsString::from("--lib=whitaker_suite"),
            OsString::from("--no-metadata"),
            OsString::from("--"),
            OsString::from("-p"),
            OsString::from("member_a"),
            OsString::from("-q"),
        ]],
        "the listing must not touch the assembled Cargo arguments"
    );
}

/// Keep the run alive when the workspace cannot be read.
#[test]
fn a_metadata_failure_does_not_stop_the_run() {
    let port = ScriptedPort::answering(Ok(()));
    let arguments = crate::cli::check::CheckArgs {
        no_install: false,
        cargo: CargoOptions::default(),
        cargo_args: Vec::new(),
    };
    let mut stderr = Vec::new();
    let outcome = run_check_command(
        CheckRequest::new(
            &arguments,
            DiagnosticSettings {
                quiet: false,
                verbosity: 1,
            },
            Some(Utf8Path::new("/work/app")),
        ),
        &NoOpLocalizer::new(),
        CheckAdapters {
            check: &port,
            packages: &port,
        },
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "a metadata failure is advisory and must not fail the run"
    );
    let text = String::from_utf8(stderr).expect("stderr is UTF-8");
    assert!(
        text.contains("could not list the packages Cargo will check"),
        "the unavailable listing must be noted, got:\n{text}"
    );
}
