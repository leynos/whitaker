//! The branch one `check` invocation took.
//!
//! The vocabulary is closed and small, so an operator can group invocations by
//! it and it is safe as a metric label: it never follows user input. The
//! command layer, which decides the branch, and the adapter telemetry, which
//! records it, share this type rather than a bare string, so adding, renaming
//! or removing a branch is a compile-time change on both sides.

/// The branch one `check` invocation took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// A completed lint run: the user received a lint result.
    Linted,
    /// Configuration failed before readiness could be judged.
    ConfigurationFailed,
    /// Provisioning was refused, so nothing was installed.
    Refused,
    /// A repair ran and failed.
    RepairFailed,
    /// A repair ran, but the installation is still unready.
    NotConverged,
    /// A host fault that installing again cannot fix.
    HostFailed,
    /// The Dylint run could not be started.
    RunFailed,
    /// Whitaker's own output could not be written.
    OutputFailed,
}

impl Decision {
    /// Return the bounded label this decision records.
    ///
    /// The labels are metric and event labels, so they are stable identifiers
    /// rather than prose, and they keep the flow's own spelling.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::Decision;
    ///
    /// assert_eq!(Decision::Linted.label(), "linted");
    /// assert_eq!(Decision::NotConverged.label(), "not-converged");
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Linted => "linted",
            Self::ConfigurationFailed => "configuration-failed",
            Self::Refused => "refused",
            Self::RepairFailed => "repair-failed",
            Self::NotConverged => "not-converged",
            Self::HostFailed => "host-failed",
            Self::RunFailed => "run-failed",
            Self::OutputFailed => "output-failed",
        }
    }

    /// Report whether this decision means the user received a lint result.
    ///
    /// Every other branch means they did not, which is what decides whether
    /// the event is reported at `warn` rather than `info`.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::Decision;
    ///
    /// assert!(Decision::Linted.is_lint_result());
    /// assert!(!Decision::HostFailed.is_lint_result());
    /// ```
    #[must_use]
    pub const fn is_lint_result(self) -> bool { matches!(self, Self::Linted) }
}
