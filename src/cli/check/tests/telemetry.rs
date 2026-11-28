//! What the command boundary records for the readiness answers it consumes.
//!
//! Recording wraps the ports the flow drives, so the flow's second readiness
//! question — the one after a repair — is counted too. Driving the command is
//! what proves the wrapper sits on the production path; the recording
//! functions and their bounded labels are pinned by the adapter's telemetry
//! tests.

use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use ortho_config::NoOpLocalizer;
use tracing::subscriber::with_default;
use tracing_subscriber::fmt::MakeWriter;

use super::{ScriptedPort, arguments, dylint_version, not_installed};
use crate::{
    adapters::check::telemetry::CHECK_OUTCOMES,
    cli::check::{CheckAdapters, CheckRequest, Outcome, exit::CheckOutcome, run_check_command},
    diagnostics::DiagnosticSettings,
};

/// A writer that keeps what a subscriber formatted, for a test to read.
#[derive(Clone, Default)]
struct EventBuffer(Arc<Mutex<Vec<u8>>>);

impl EventBuffer {
    /// Return everything written so far as text.
    fn written(&self) -> String {
        let bytes = self.0.lock().expect("the buffer is not poisoned").clone();
        String::from_utf8(bytes).expect("the formatter writes UTF-8")
    }
}

impl Write for EventBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("the buffer is poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl<'writer> MakeWriter<'writer> for EventBuffer {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer { self.clone() }
}

/// Record one event per readiness answer, including the post-repair one.
///
/// A machine with no installation is repaired and then checked again, so the
/// flow asks readiness twice. Both answers are decisions the flow acted on,
/// and a wrapper around the ports sees each exactly once; recording only the
/// flow's first question would silently drop the second.
#[test]
fn each_readiness_answer_reaches_the_subscriber_once() {
    let buffer = EventBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(buffer.clone())
        .finish();
    // The last answer repeats, so the second question finds the repair ready.
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]);

    let outcome = with_default(subscriber, || {
        run_check_command(
            CheckRequest::new(&arguments(&[]), DiagnosticSettings::default(), None),
            &NoOpLocalizer::new(),
            CheckAdapters {
                check: &port,
                packages: &port,
            },
            &mut io::sink(),
        )
    });

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "the repaired installation must lint"
    );
    let written = buffer.written();
    assert_eq!(
        written.matches("preflight=").count(),
        2,
        "each of the two readiness answers must be recorded once: {written}"
    );
    let first = written.find("preflight=\"not_installed\"");
    let second = written.find("preflight=\"ready\"");
    assert!(
        matches!((first, second), (Some(earlier), Some(later)) if earlier < later),
        "the answers must be recorded in the order the flow consumed them: {written}"
    );
}

/// Return whether `name` was counted once under `label` = `value`.
fn counted_with(
    recorded: &[(
        CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )],
    name: &str,
    label: &str,
    value: &str,
) -> bool {
    recorded.iter().any(|(key, _, _, recorded_value)| {
        let matches_key = key.key().name() == name
            && key
                .key()
                .labels()
                .any(|entry| entry.key() == label && entry.value() == value);
        matches_key && matches!(recorded_value, DebugValue::Counter(1))
    })
}

/// Count and warn when a repair leaves the installation broken.
///
/// A refusal has its own counter, but an outcome that fails after readiness
/// was settled must be visible as a bounded counter and at `warn` level, so an
/// operator can alert on it without a debug subscriber.
#[test]
fn an_operational_failure_is_counted_and_reported_at_warn() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let buffer = EventBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(buffer.clone())
        .finish();
    // The repair succeeds, and the second readiness answer is still a defect.
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Err(dylint_version())]);

    let outcome = metrics::with_local_recorder(&recorder, || {
        with_default(subscriber, || {
            run_check_command(
                CheckRequest::new(&arguments(&[]), DiagnosticSettings::default(), None),
                &NoOpLocalizer::new(),
                CheckAdapters {
                    check: &port,
                    packages: &port,
                },
                &mut io::sink(),
            )
        })
    });

    assert!(
        matches!(outcome, Outcome::Check(CheckOutcome::NotConverged(_))),
        "the repair must leave a defect the flow reports as not converged: {outcome:?}"
    );
    let written = buffer.written();
    let event = written
        .lines()
        .find(|line| line.contains("decision=\"not-converged\""))
        .unwrap_or_default();
    assert!(
        event.contains(" WARN "),
        "an operational failure must be reported at warn: {event}"
    );
    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_with(&captured, CHECK_OUTCOMES, "decision", "not-converged"),
        "an operational failure must produce a non-zero counter: {captured:#?}"
    );
}

/// Report the decision event inside the invocation's own span.
///
/// The decision belongs to one invocation, so it must be emitted while the
/// `whitaker.check` span is current: recorded afterwards it would lose the
/// span's fields and could not be correlated with the readiness answers the
/// same invocation reported.
#[test]
fn the_decision_event_is_recorded_inside_the_check_span() {
    let buffer = EventBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(buffer.clone())
        .finish();
    let port = ScriptedPort::answering(Ok(()));

    let outcome = with_default(subscriber, || {
        run_check_command(
            CheckRequest::new(&arguments(&[]), DiagnosticSettings::default(), None),
            &NoOpLocalizer::new(),
            CheckAdapters {
                check: &port,
                packages: &port,
            },
            &mut io::sink(),
        )
    });

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "a ready port must lint"
    );
    let written = buffer.written();
    let decision = written
        .lines()
        .find(|line| line.contains("decision=\"linted\""))
        .unwrap_or_default();
    assert!(
        decision.contains("whitaker.check{"),
        "the decision must be emitted inside the invocation span: {decision}"
    );
}
