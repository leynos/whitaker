//! The wording `whitaker check` uses to explain why it could not lint.
//!
//! Two things here are contracts rather than prose. The `whitaker: error[...]`
//! prefix and its bracketed key are what automation matches, so the key comes
//! from [`reason_key`] and never from the sentence. The sentence itself is for
//! the person reading the terminal, and it says what is wrong in terms of the
//! thing that is wrong, not in terms of the error type that reported it.

use std::io::Write;

use super::{
    classify::{HostFault, RepairableReason},
    exit::{CheckOutcome, reason_key},
};

/// One diagnostic, split into the parts the two output lines need.
struct Diagnostic {
    /// The stable key printed inside the brackets.
    key: &'static str,
    /// What is wrong, as one sentence without a trailing full stop.
    message: String,
    /// What the user can do about it, when Whitaker knows.
    advice: Option<&'static str>,
}

/// Write the diagnostic an operational outcome deserves.
///
/// A lint run writes nothing: Dylint has already reported its own result to
/// the user's terminal, and repeating it would only add noise.
///
/// # Errors
///
/// Returns the write error when standard error cannot be written, so the
/// caller can report `exec.output` instead of claiming a cause it never
/// managed to print.
pub fn report(outcome: CheckOutcome, stderr: &mut dyn Write) -> std::io::Result<()> {
    let Some(diagnostic) = diagnostic(outcome) else {
        return Ok(());
    };
    writeln!(
        stderr,
        "whitaker: error[{}]: {}",
        diagnostic.key, diagnostic.message
    )?;
    if let Some(advice) = diagnostic.advice {
        writeln!(stderr, "help: {advice}")?;
    }
    Ok(())
}

/// Build the diagnostic for every outcome that has one.
fn diagnostic(outcome: CheckOutcome) -> Option<Diagnostic> {
    let key = reason_key(&outcome)?;
    let message = match outcome {
        // A configuration failure carries no reason key yet: its own
        // diagnostics arrive with the selector work in 3.5.4.
        CheckOutcome::Linted(_) | CheckOutcome::ConfigurationFailed => return None,
        CheckOutcome::Refused(reason) => reason_sentence(reason).to_owned(),
        CheckOutcome::RepairFailed => "installing Whitaker did not complete".to_owned(),
        // The wording must fit both ways this outcome arises: a repair that
        // ran and did not converge, and a lint run that found the approved
        // installation had become unusable. Only the first installed anything.
        CheckOutcome::NotConverged(reason) => format!(
            "the Whitaker installation is not ready: {}",
            reason_sentence(reason)
        ),
        CheckOutcome::HostFailed(fault) => fault_sentence(fault).to_owned(),
        CheckOutcome::RunFailed => "`cargo dylint` could not be started".to_owned(),
        CheckOutcome::OutputFailed => "Whitaker could not write its own output".to_owned(),
    };
    Some(Diagnostic {
        key,
        message,
        advice: advice(outcome),
    })
}

/// Say what is missing or stale about the installation.
const fn reason_sentence(reason: RepairableReason) -> &'static str {
    match reason {
        RepairableReason::NotInstalled => "Whitaker is not installed",
        RepairableReason::Metadata => "the recorded Whitaker bootstrap metadata cannot be used",
        RepairableReason::LibrariesMissing => "the installed Whitaker lint libraries are missing",
        RepairableReason::Snapshot => "the recorded Clippy snapshot is missing or invalid",
        RepairableReason::CargoDylint => "`cargo dylint` could not be queried",
        RepairableReason::DylintVersion => {
            "the installed `cargo dylint` is not the release Whitaker was built for"
        }
        RepairableReason::UserPath => {
            "the Dylint library path you set does not name exactly one Whitaker suite library"
        }
    }
}

/// Say what failed on the host.
const fn fault_sentence(fault: HostFault) -> &'static str {
    match fault {
        HostFault::DataDirectory => "the platform data directory could not be determined",
        HostFault::Io => "the filesystem failed while inspecting the Whitaker installation",
        HostFault::Lock => "the Whitaker installation lock could not be taken",
        HostFault::Spawn => "`cargo dylint` could not be started",
    }
}

/// Offer the next step, for the outcomes Whitaker can suggest one for.
const fn advice(outcome: CheckOutcome) -> Option<&'static str> {
    match outcome {
        CheckOutcome::Refused(RepairableReason::UserPath) => Some(concat!(
            "Whitaker will not change a Dylint library path you set; ",
            "add the Whitaker suite to it, or unset it and run `whitaker install`",
        )),
        CheckOutcome::Refused(_) => Some("run `whitaker install` to provision Whitaker"),
        CheckOutcome::RepairFailed | CheckOutcome::NotConverged(_) => {
            Some("run `whitaker install` to see the installer's diagnostics")
        }
        CheckOutcome::ConfigurationFailed
        | CheckOutcome::HostFailed(_)
        | CheckOutcome::RunFailed
        | CheckOutcome::OutputFailed
        | CheckOutcome::Linted(_) => None,
    }
}

#[cfg(test)]
mod tests {
    //! The rendered diagnostics, as one reviewed snapshot.

    use super::report;
    use crate::cli::check::{
        classify::{HostFault, RepairableReason},
        exit::CheckOutcome,
    };

    /// Every outcome that reports something, in a stable order.
    const OUTCOMES: [CheckOutcome; 18] = [
        CheckOutcome::Linted(0),
        CheckOutcome::Linted(23),
        CheckOutcome::ConfigurationFailed,
        CheckOutcome::Refused(RepairableReason::NotInstalled),
        CheckOutcome::Refused(RepairableReason::Metadata),
        CheckOutcome::Refused(RepairableReason::LibrariesMissing),
        CheckOutcome::Refused(RepairableReason::Snapshot),
        CheckOutcome::Refused(RepairableReason::CargoDylint),
        CheckOutcome::Refused(RepairableReason::DylintVersion),
        CheckOutcome::Refused(RepairableReason::UserPath),
        CheckOutcome::RepairFailed,
        CheckOutcome::NotConverged(RepairableReason::Snapshot),
        CheckOutcome::HostFailed(HostFault::DataDirectory),
        CheckOutcome::HostFailed(HostFault::Io),
        CheckOutcome::HostFailed(HostFault::Lock),
        CheckOutcome::HostFailed(HostFault::Spawn),
        CheckOutcome::RunFailed,
        CheckOutcome::OutputFailed,
    ];

    /// Render every outcome into one buffer, in the table's order.
    fn render_all() -> String {
        let mut rendered = Vec::new();
        for outcome in OUTCOMES {
            report(outcome, &mut rendered).expect("writing to a Vec cannot fail");
        }
        String::from_utf8(rendered).expect("every diagnostic is UTF-8")
    }

    #[test]
    fn renders_every_diagnostic() {
        insta::assert_snapshot!(render_all());
    }

    #[test]
    fn a_lint_run_reports_nothing_of_its_own() {
        let mut rendered = Vec::new();

        report(CheckOutcome::Linted(1), &mut rendered).expect("writing to a Vec cannot fail");

        assert!(
            rendered.is_empty(),
            "a lint run's status is Dylint's to report, got {rendered:?}"
        );
    }

    #[test]
    fn every_reason_key_appears_in_brackets_verbatim() {
        let rendered = render_all();

        for key in [
            "provision.refused",
            "provision.user-path",
            "provision.repair-failed",
            "provision.not-converged",
            "host.lock",
        ] {
            assert!(
                rendered.contains(&format!("whitaker: error[{key}]:")),
                "missing the bracketed key {key} in {rendered:?}"
            );
        }
    }

    #[test]
    fn a_failing_stream_is_reported_rather_than_swallowed() {
        struct Closed;

        impl std::io::Write for Closed {
            fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("closed"))
            }

            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        let error = report(
            CheckOutcome::Refused(RepairableReason::NotInstalled),
            &mut Closed,
        );

        assert!(
            error.is_err(),
            "a diagnostic that never reached its stream must be reported"
        );
    }
}
