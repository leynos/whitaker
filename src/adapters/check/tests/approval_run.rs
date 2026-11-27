//! What the adapter does with an approval when a run consumes it.
//!
//! This drives `CheckServices::check_run`, so it is about the adapter's
//! dispatch rather than the holder's state transitions. The state-transition
//! cases live in `approval` and `approval_supersession`; this one keeps the
//! process-launch scaffolding separate.
//!
//! It is Unix-only because it needs an executable stub to observe whether a
//! child was started at all.

#![cfg(unix)]

use super::{
    super::{Approval, prepare_direct_library_path},
    fixtures::{directory_with_suite, suite_filename},
    recording_cargo::RecordingCargo,
};
use crate::{
    adapters::SystemServices,
    ports::{CheckPrepared, CheckRunError, CheckServices},
};

/// Refuse a second run of the same direct value through `check_run`.
///
/// The direct route has no held run, so the only record that the value was
/// spent is the approval number. A second `check_run` with the same value must
/// be refused before Dylint starts rather than linting again unapproved.
///
/// The launcher is a recording stub rather than the `cargo` on `PATH`, so the
/// launch count is observable: a refusal that still spawned Dylint would pass
/// an assertion on the returned error alone.
#[test]
fn a_direct_value_runs_once_through_check_run() {
    let cargo = RecordingCargo::new();
    let mut services = SystemServices::new();
    services.cargo = cargo.program().to_owned();
    let directory = directory_with_suite();
    let path = prepare_direct_library_path(directory.path().as_os_str().to_owned())
        .expect("the staged suite makes the path ready");
    let prepared = Approval::Direct(path).record(&services.approved);
    let CheckPrepared::Direct { .. } = &prepared else {
        panic!("a direct path must approve the direct route");
    };

    assert_eq!(
        cargo.starts(),
        0,
        "no child may start before the approved value runs"
    );
    services
        .check_run(prepared.clone(), &[], &mut Vec::new())
        .expect("the first run must be allowed");
    // Read between the two calls: a count taken only at the end would be one
    // whether the first call launched and the second did not, or the reverse.
    assert_eq!(
        cargo.starts(),
        1,
        "the approved value must launch Dylint exactly once"
    );
    // The suite is then removed, so a second run that reached the re-check
    // would be refused for that reason instead of for the spent approval.
    std::fs::remove_file(directory.path().join(suite_filename()))
        .expect("the approved suite can be removed");

    let second = services.check_run(prepared, &[], &mut Vec::new());

    assert!(
        matches!(second, Err(CheckRunError::Unapproved)),
        "a direct value must not run twice, got {second:?}"
    );
    assert_eq!(
        cargo.starts(),
        1,
        "a refused value must not launch a second Dylint child"
    );
}
