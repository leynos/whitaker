//! The wording `whitaker check` uses to explain why it could not lint.
//!
//! Two things here are contracts rather than prose. The `whitaker: error[...]`
//! prefix and its bracketed key are what automation matches, so the key comes
//! from [`reason_key`] and never from the sentence. The sentence itself is for
//! the person reading the terminal, and it says what is wrong in terms of the
//! thing that is wrong, not in terms of the error type that reported it.
//!
//! The sentences are localizable: each carries a stable message id, and the
//! `en-GB` wording here is the fallback a localizer that declines to translate
//! receives. The reason key is never localized, because automation reads it.

use std::{collections::HashMap, io::Write};

use ortho_config::{FluentValue, LocalizationArgs, Localizer};

use super::{
    classify::{HostFault, RepairableReason},
    exit::{CheckOutcome, reason_key},
};

/// A localizable sentence and the `en-GB` wording a localizer falls back to.
#[derive(Debug, Clone, Copy)]
struct Message {
    /// The id a translation resource keys on.
    id: &'static str,
    /// The sentence to print when no translation exists.
    fallback: &'static str,
}

impl Message {
    /// Name one localizable sentence.
    const fn new(id: &'static str, fallback: &'static str) -> Self { Self { id, fallback } }
}

/// One diagnostic, split into the parts the two output lines need.
struct Diagnostic {
    /// The stable key printed inside the brackets.
    key: &'static str,
    /// What is wrong, as one sentence without a trailing full stop.
    message: Message,
    /// A reason sentence the message introduces, printed in its place.
    reason: Option<RepairableReason>,
    /// What the user can do about it, when Whitaker knows.
    advice: Option<Message>,
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
pub fn report(
    outcome: CheckOutcome,
    localizer: &dyn Localizer,
    stderr: &mut dyn Write,
) -> std::io::Result<()> {
    let Some(diagnostic) = diagnostic(outcome) else {
        return Ok(());
    };
    writeln!(
        stderr,
        "whitaker: error[{}]: {}",
        diagnostic.key,
        render(&diagnostic, localizer)
    )?;
    if let Some(advice) = diagnostic.advice {
        writeln!(stderr, "help: {}", translate(&advice, None, localizer))?;
    }
    Ok(())
}

/// Render a diagnostic's sentence, folding in the reason it introduces.
fn render(diagnostic: &Diagnostic, localizer: &dyn Localizer) -> String {
    diagnostic.reason.map_or_else(
        || translate(&diagnostic.message, None, localizer),
        |introduced| {
            let sentence = reason_message(introduced);
            translate(&diagnostic.message, Some(&sentence), localizer)
        },
    )
}

/// Look one sentence up, and compose it with `reason` when it has one.
///
/// A translated wrapper may place the reason anywhere in its sentence, so the
/// reason travels as a Fluent argument rather than being concatenated here.
fn translate(message: &Message, reason: Option<&Message>, localizer: &dyn Localizer) -> String {
    let Some(introduced) = reason else {
        return localizer.message(message.id, None, message.fallback);
    };
    let reason_text = localizer.message(introduced.id, None, introduced.fallback);
    let args: LocalizationArgs<'_> =
        HashMap::from([("reason", FluentValue::from(reason_text.as_str()))]);
    let fallback = format!("{}: {reason_text}", message.fallback);
    localizer.message(message.id, Some(&args), &fallback)
}

/// Build the diagnostic for every outcome that has one.
fn diagnostic(outcome: CheckOutcome) -> Option<Diagnostic> {
    let key = reason_key(&outcome)?;
    let (message, reason, advice) = match outcome {
        // A configuration failure carries no reason key yet: its own
        // diagnostics arrive with the selector work in 3.5.4.
        CheckOutcome::Linted(_) | CheckOutcome::ConfigurationFailed => return None,
        // The reason sentence is the diagnostic, so it is not repeated after
        // the message: a refusal says what is wrong exactly once.
        CheckOutcome::Refused(failure) => (reason_message(failure), None, advice(outcome)),
        CheckOutcome::RepairFailed => (
            Message::new(
                "check-repair-failed",
                "installing Whitaker did not complete",
            ),
            None,
            advice(outcome),
        ),
        // The wording must fit both ways this outcome arises: a repair that
        // ran and did not converge, and a lint run that found the approved
        // installation had become unusable. Only the first installed anything.
        CheckOutcome::NotConverged(failure) => (
            Message::new(
                "check-not-converged",
                "the Whitaker installation is not ready",
            ),
            Some(failure),
            advice(outcome),
        ),
        CheckOutcome::HostFailed(fault) => (fault_message(fault), None, advice(outcome)),
        CheckOutcome::RunFailed => (
            Message::new("check-run-failed", "`cargo dylint` could not be started"),
            None,
            None,
        ),
        CheckOutcome::OutputFailed => (
            Message::new(
                "check-output-failed",
                "Whitaker could not write its own output",
            ),
            None,
            None,
        ),
    };
    Some(Diagnostic {
        key,
        message,
        reason,
        advice,
    })
}

/// Say what is missing or stale about the installation.
const fn reason_message(reason: RepairableReason) -> Message {
    match reason {
        RepairableReason::NotInstalled => {
            Message::new("check-not-installed", "Whitaker is not installed")
        }
        RepairableReason::Metadata => Message::new(
            "check-metadata-unusable",
            "the recorded Whitaker bootstrap metadata cannot be used",
        ),
        RepairableReason::LibrariesMissing => Message::new(
            "check-libraries-missing",
            "the installed Whitaker lint libraries are missing",
        ),
        RepairableReason::Snapshot => Message::new(
            "check-snapshot-invalid",
            "the recorded Clippy snapshot is missing or invalid",
        ),
        RepairableReason::CargoDylint => Message::new(
            "check-cargo-dylint-unqueried",
            "`cargo dylint` could not be queried",
        ),
        RepairableReason::DylintVersion => Message::new(
            "check-dylint-version-mismatch",
            "the installed `cargo dylint` is not the release Whitaker was built for",
        ),
        RepairableReason::UserPath => Message::new(
            "check-user-path-ambiguous",
            "the Dylint library path you set does not name exactly one Whitaker suite library",
        ),
    }
}

/// Say what failed on the host.
const fn fault_message(fault: HostFault) -> Message {
    match fault {
        HostFault::DataDirectory => Message::new(
            "check-data-directory-unknown",
            "the platform data directory could not be determined",
        ),
        HostFault::Io => Message::new(
            "check-filesystem-failed",
            "the filesystem failed while inspecting the Whitaker installation",
        ),
        HostFault::Lock => Message::new(
            "check-lock-unavailable",
            "the Whitaker installation lock could not be taken",
        ),
        HostFault::Spawn => {
            Message::new("check-spawn-failed", "`cargo dylint` could not be started")
        }
    }
}

/// Offer the next step, for the outcomes Whitaker can suggest one for.
const fn advice(outcome: CheckOutcome) -> Option<Message> {
    match outcome {
        CheckOutcome::Refused(RepairableReason::UserPath) => Some(Message::new(
            "check-advice-user-path",
            concat!(
                "Whitaker will not change a Dylint library path you set; ",
                "add the Whitaker suite to it, or unset it and run `whitaker install`",
            ),
        )),
        CheckOutcome::Refused(_) => Some(Message::new(
            "check-advice-install",
            "run `whitaker install` to provision Whitaker",
        )),
        CheckOutcome::RepairFailed | CheckOutcome::NotConverged(_) => Some(Message::new(
            "check-advice-installer-diagnostics",
            "run `whitaker install` to see the installer's diagnostics",
        )),
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

    use ortho_config::{LocalizationArgs, Localizer, NoOpLocalizer};

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
            report(outcome, &NoOpLocalizer::new(), &mut rendered)
                .expect("writing to a Vec cannot fail");
        }
        String::from_utf8(rendered).expect("every diagnostic is UTF-8")
    }

    /// Render every reporting outcome as one reviewed snapshot.
    #[test]
    fn renders_every_diagnostic() {
        insta::assert_snapshot!(render_all());
    }

    /// Report nothing of Whitaker's own after a lint run.
    #[test]
    fn a_lint_run_reports_nothing_of_its_own() {
        let mut rendered = Vec::new();

        report(
            CheckOutcome::Linted(1),
            &NoOpLocalizer::new(),
            &mut rendered,
        )
        .expect("writing to a Vec cannot fail");

        assert!(
            rendered.is_empty(),
            "a lint run's status is Dylint's to report, got {rendered:?}"
        );
    }

    /// Keep every automation key inside the brackets, verbatim.
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

    /// A localizer that translates one sentence and declines the rest.
    struct Partial;

    impl Localizer for Partial {
        fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
            (id == "check-not-installed").then(|| "Nid yw Whitaker wedi'i osod".to_owned())
        }
    }

    /// Keep the key verbatim while translating the sentence it introduces.
    #[test]
    fn a_translated_sentence_keeps_the_key_automation_reads() {
        let mut rendered = Vec::new();

        report(
            CheckOutcome::Refused(RepairableReason::NotInstalled),
            &Partial,
            &mut rendered,
        )
        .expect("writing to a Vec cannot fail");
        let text = String::from_utf8(rendered).expect("every diagnostic is UTF-8");

        assert!(
            text.starts_with("whitaker: error[provision.refused]: Nid yw Whitaker"),
            "the sentence is localized while the key stays verbatim: {text:?}"
        );
    }

    /// Fall back to the en-GB wording when a translation is declined.
    #[test]
    fn an_untranslated_sentence_falls_back_to_english() {
        let mut rendered = Vec::new();

        report(
            CheckOutcome::Refused(RepairableReason::Metadata),
            &Partial,
            &mut rendered,
        )
        .expect("writing to a Vec cannot fail");
        let text = String::from_utf8(rendered).expect("every diagnostic is UTF-8");

        assert!(
            text.contains("the recorded Whitaker bootstrap metadata cannot be used"),
            "a localizer that declines a message must leave the en-GB wording: {text:?}"
        );
    }

    /// Surface a diagnostic that never reached its stream.
    #[test]
    fn a_failing_stream_is_reported_rather_than_swallowed() {
        /// A stream that rejects every write, standing in for a closed stderr.
        struct Closed;

        impl std::io::Write for Closed {
            /// Reject the write, as a closed descriptor does.
            fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("closed"))
            }

            /// Complete without work: nothing was buffered to flush.
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        let error = report(
            CheckOutcome::Refused(RepairableReason::NotInstalled),
            &NoOpLocalizer::new(),
            &mut Closed,
        );

        assert!(
            error.is_err(),
            "a diagnostic that never reached its stream must be reported"
        );
    }
}
