//! Kani-only samplers shared by the check proof suites.
//!
//! The flow and exit proofs both range over every repairable reason and host
//! fault. Keeping one pair of samplers here means a new variant is covered by
//! both suites or by neither, rather than drifting between two wildcard
//! tables that the compiler cannot compare.

use crate::cli::check::classify::{HostFault, RepairableReason};

/// Pick one of the eight repairable reasons.
pub(crate) fn any_reason() -> RepairableReason {
    match kani::any::<u8>() {
        0 => RepairableReason::NotInstalled,
        1 => RepairableReason::Metadata,
        2 => RepairableReason::LibrariesMissing,
        3 => RepairableReason::LibrariesAmbiguous,
        4 => RepairableReason::Snapshot,
        5 => RepairableReason::CargoDylint,
        6 => RepairableReason::DylintVersion,
        _ => RepairableReason::UserPath,
    }
}

/// Pick one of the four host faults.
pub(crate) fn any_fault() -> HostFault {
    match kani::any::<u8>() {
        0 => HostFault::DataDirectory,
        1 => HostFault::Io,
        2 => HostFault::Lock,
        _ => HostFault::Spawn,
    }
}
