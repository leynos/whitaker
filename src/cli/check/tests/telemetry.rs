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

use ortho_config::NoOpLocalizer;
use tracing::subscriber::with_default;
use tracing_subscriber::fmt::MakeWriter;

use super::{ScriptedPort, arguments, not_installed};
use crate::{
    cli::check::{CheckRequest, Outcome, exit::CheckOutcome, run_check_command},
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
            CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
            &NoOpLocalizer::new(),
            &port,
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
