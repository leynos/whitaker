//! The answers a scripted `check` port gives, for the Kani harnesses.
//!
//! Separated from the invariants so the flow's proof modules state properties
//! rather than describe the port: a script is data, and both the exhaustive
//! harnesses and the reachability witnesses drive the same type.

/// One scripted readiness answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    /// Dylint can run against this installation.
    Ready,
    /// Provisioning could make the installation usable.
    Repairable,
    /// The host failed before readiness could be decided.
    Host,
}

/// One scripted provisioning result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepairAnswer {
    /// Provisioning completed.
    Ok,
    /// Provisioning ran and failed.
    Failed,
    /// Whitaker could not write its own output.
    Output,
    /// The host failed before provisioning could start.
    Host,
}

/// One scripted lint result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunAnswer {
    /// Dylint ran and reported a status.
    Lint,
    /// The installation stopped being ready before the run.
    Provisioning,
    /// `cargo dylint` could not be started.
    Spawn,
    /// Whitaker could not write its own output.
    Output,
    /// The host failed while Dylint was being started or run.
    Host,
}

/// Every answer a scripted port will give, in one value.
///
/// Keeping the answers in one script means the helpers that build a port take
/// one argument instead of a list of primitives.
#[derive(Debug, Clone, Copy)]
pub(super) struct Script {
    /// The answer each readiness check gives, by call index.
    pub(super) answers: [Answer; 2],
    /// Which result the repair reports.
    pub(super) repair: RepairAnswer,
    /// Which result the run reports, by call index.
    pub(super) runs: [RunAnswer; 2],
}

/// Assemble a scripted port's answers.
pub(super) const fn script(
    answers: [Answer; 2],
    repair: RepairAnswer,
    runs: [RunAnswer; 2],
) -> Script {
    Script {
        answers,
        repair,
        runs,
    }
}

impl Script {
    /// Build a script whose every answer is a symbolic choice.
    pub(super) fn symbolic() -> Self {
        Self {
            answers: [any_answer(), any_answer()],
            repair: any_repair(),
            runs: [any_run(), any_run()],
        }
    }
}

/// Sample one readiness answer.
fn any_answer() -> Answer {
    match kani::any::<u8>() {
        0 => Answer::Ready,
        1 => Answer::Repairable,
        _ => Answer::Host,
    }
}

/// Sample one provisioning result.
fn any_repair() -> RepairAnswer {
    match kani::any::<u8>() {
        0 => RepairAnswer::Ok,
        1 => RepairAnswer::Failed,
        2 => RepairAnswer::Output,
        _ => RepairAnswer::Host,
    }
}

/// Sample one lint result.
fn any_run() -> RunAnswer {
    match kani::any::<u8>() {
        0 => RunAnswer::Lint,
        1 => RunAnswer::Provisioning,
        2 => RunAnswer::Spawn,
        3 => RunAnswer::Output,
        _ => RunAnswer::Host,
    }
}
