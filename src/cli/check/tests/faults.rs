//! How a port failure becomes the flow's own verdict.
//!
//! The flow never sees a port error: the command classifies it first, and the
//! class decides whether the installation may be repaired, whether the host
//! is at fault, and which reason key the user is told. Each case here pins one
//! arm of that table, so an arm that was removed, inverted or misrouted fails.

use std::io;

use rstest::rstest;

use super::super::{RepairFault, RunFault, repair_fault, run_fault};
use crate::ports::{
    BootstrapFault,
    CheckPreflightError,
    CheckRepairError,
    CheckRunError,
    HostFault,
    RepairableReason,
};

/// Classify every repair failure into the verdict the flow acts on.
#[rstest]
#[case::install(
    CheckRepairError::Install { message: "cargo failed".to_owned() },
    RepairFault::Failed
)]
#[case::lock(
    CheckRepairError::Lock { message: "held elsewhere".to_owned() },
    RepairFault::Host(HostFault::Lock)
)]
#[case::data_directory(
    CheckRepairError::DataDirectory,
    RepairFault::Host(HostFault::DataDirectory)
)]
#[case::output(
    CheckRepairError::Output { source: io::Error::other("closed") },
    RepairFault::Output
)]
#[case::host(CheckRepairError::Host { message: "unreadable".to_owned() }, RepairFault::Host(HostFault::Io))]
fn a_repair_failure_is_classified_by_whether_retrying_helps(
    #[case] error: CheckRepairError,
    #[case] expected: RepairFault,
) {
    assert_eq!(
        repair_fault(&error),
        expected,
        "a repair failure must land on its own arm of the table"
    );
}

/// Classify every run failure into the verdict the flow acts on.
#[rstest]
#[case::provisioning(
    CheckRunError::Bootstrap(BootstrapFault::NotInstalled { path: "bootstrap.json".to_owned() }),
    RunFault::Provisioning(RepairableReason::NotInstalled)
)]
#[case::host(
    CheckRunError::Bootstrap(BootstrapFault::Io { path: "lints".to_owned(), reason: "denied".to_owned() }),
    RunFault::Host(HostFault::Io)
)]
#[case::unapproved(CheckRunError::Unapproved, RunFault::Spawn)]
#[case::recheck_ambiguous(
    CheckRunError::Preflight(CheckPreflightError::RecordedPathAmbiguous {
        path: "lints/lib".to_owned(),
    }),
    RunFault::Provisioning(RepairableReason::LibrariesAmbiguous)
)]
#[case::recheck_missing(
    CheckRunError::Preflight(CheckPreflightError::Bootstrap(
        BootstrapFault::LibrariesMissing { path: "lints/lib".to_owned() }
    )),
    RunFault::Provisioning(RepairableReason::LibrariesMissing)
)]
#[case::recheck_user_path_missing(
    CheckRunError::Preflight(CheckPreflightError::UserPathHasNoSuite),
    RunFault::Provisioning(RepairableReason::UserPath)
)]
#[case::recheck_user_path_ambiguous(
    CheckRunError::Preflight(CheckPreflightError::UserPathAmbiguous),
    RunFault::Provisioning(RepairableReason::UserPath)
)]
#[case::spawn(
    CheckRunError::Spawn { source: io::Error::other("no cargo") },
    RunFault::Spawn
)]
#[case::output(
    CheckRunError::Output { source: io::Error::other("closed") },
    RunFault::Output
)]
fn a_run_failure_is_classified_by_what_can_fix_it(
    #[case] error: CheckRunError,
    #[case] expected: RunFault,
) {
    assert_eq!(
        run_fault(error),
        expected,
        "a run failure must land on its own arm of the table"
    );
}
