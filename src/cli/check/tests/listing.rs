//! The `-v` coverage listing through the real command boundary.

use std::ffi::OsString;

use camino::Utf8Path;
use ortho_config::NoOpLocalizer;
use rstest::rstest;

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
            targets: Vec::new(),
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

/// Options that select the whole workspace.
fn workspace_options() -> CargoOptions {
    CargoOptions {
        workspace: true,
        ..CargoOptions::default()
    }
}

/// Options that use the deprecated workspace alias.
fn alias_options() -> CargoOptions {
    CargoOptions {
        all: true,
        ..CargoOptions::default()
    }
}

/// Show when the listing and the deprecation notice are printed.
///
/// Both are diagnostics, so one table states the rule: `-v` prints the
/// listing, `-q` suppresses the listing and the notice, and `--all` prints the
/// notice at the default verbosity. The listing's own shape is asserted in the
/// verbose case, where it is actually written.
#[rstest]
#[case::verbose(
    DiagnosticSettings {
        quiet: false,
        verbosity: 1,
    },
    workspace_options(),
    true,
    false
)]
#[case::default_verbosity(DiagnosticSettings::default(), workspace_options(), false, false)]
#[case::quietly_verbose(
    DiagnosticSettings {
        quiet: true,
        verbosity: 1,
    },
    workspace_options(),
    false,
    false
)]
#[case::deprecated_alias(DiagnosticSettings::default(), alias_options(), false, true)]
#[case::quiet_alias(
    DiagnosticSettings {
        quiet: true,
        verbosity: 0,
    },
    alias_options(),
    false,
    false
)]
fn the_listing_and_notice_follow_the_diagnostic_settings(
    #[case] settings: DiagnosticSettings,
    #[case] cargo: CargoOptions,
    #[case] lists: bool,
    #[case] notices: bool,
) {
    let (outcome, stderr) = run_with(settings, cargo);

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "a diagnostic must not change the run's outcome"
    );
    assert_eq!(
        stderr.contains("    Checking member_b v0.1.0 (/work/app/member_b)"),
        lists,
        "listing presence for {settings:?}: {stderr}"
    );
    assert_eq!(
        stderr.contains("warning: --all is deprecated in favour of --workspace"),
        notices,
        "notice presence for {settings:?}: {stderr}"
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
