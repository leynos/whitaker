//! How one `check` invocation ended, independent of any exit code.
//!
//! The flow produces this value and the command layer maps it onto a process
//! status and a reason key. Keeping the outcome here, rather than beside the
//! exit mapping, is what lets the flow describe what happened without naming
//! the CLI: the status contract in ADR 0017 belongs to the command layer and
//! changes for different reasons than the flow's own result.

use super::{HostFault, RepairableReason};

/// How a `whitaker check` invocation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Dylint ran and reported this status.
    Linted(i32),
    /// The configuration is invalid. Reserved for 3.5.4.
    ConfigurationFailed,
    /// Provisioning was needed and was not permitted.
    Refused(RepairableReason),
    /// Provisioning ran and failed.
    RepairFailed,
    /// Provisioning ran and the installation is still not ready.
    NotConverged(RepairableReason),
    /// The host failed in a way provisioning cannot fix.
    HostFailed(HostFault),
    /// Dylint could not be started.
    RunFailed,
    /// Whitaker could not write its own output.
    OutputFailed,
}
