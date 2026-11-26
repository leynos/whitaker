//! What the adapter does with an approval when a run consumes it.
//!
//! These drive `CheckServices::check_run`, so they are about the adapter's
//! dispatch rather than the holder's state transitions. The state-transition
//! cases live in the parent module; this one keeps the process-launch
//! scaffolding separate.
//!
//! Both cases are Unix-only because each needs an executable stub to observe
//! whether a child was started at all.

#![cfg(unix)]

use super::{
    super::{Approval, preflight_recorded_with_cargo, prepare_direct_library_path},
    approval::held_approval,
    fixtures::{RecordingCargo, directory_with_suite, suite_filename},
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

    services
        .check_run(prepared.clone(), &[], &mut Vec::new())
        .expect("the first run must be allowed");
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
        "only the approved value may launch Dylint"
    );
}

/// Keep a newer bootstrap approval runnable after a stale direct refusal.
///
/// A direct answer supersedes an earlier bootstrap one, and the bootstrap
/// answer that follows supersedes the direct one. Refusing the stale direct
/// value must leave the newer bootstrap approval's run untouched.
#[test]
fn a_stale_direct_refusal_keeps_the_newer_bootstrap_approval() {
    let (installation, held, _first) = held_approval();
    let directory = directory_with_suite();
    let direct = Approval::Direct(
        prepare_direct_library_path(directory.path().as_os_str().to_owned())
            .expect("the staged suite makes the path ready"),
    )
    .record(&held);
    let CheckPrepared::Direct {
        approval: stale, ..
    } = direct
    else {
        panic!("the second answer must approve the direct route");
    };
    let newer = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("the recorded installation is still ready")
    .record(&held);
    let CheckPrepared::Bootstrap {
        approval: current, ..
    } = newer
    else {
        panic!("the third answer must approve the bootstrap route");
    };

    assert!(
        matches!(held.consume(stale), Err(CheckRunError::Unapproved)),
        "the superseded direct approval must be refused"
    );
    assert!(
        held.take(current).is_ok(),
        "refusing the stale direct approval must leave the newer run available"
    );
}
