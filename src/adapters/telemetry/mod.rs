//! Bounded diagnostics for the root command's forwarding boundary.
//!
//! Every label here is drawn from a closed set. Toolchain names, directory
//! paths, and raw error strings are deliberately absent: recording them would
//! make metric cardinality depend on the user's filesystem. Values that vary
//! are recorded as counts or durations instead.
//!
//! The root binary installs no metrics exporter, so these counters are inert
//! unless a host embeds the crate and installs a recorder. They are emitted
//! anyway, matching the installer library's policy: a library may describe and
//! record metrics, but only an application may choose where they go.

use std::time::Duration;

use metrics::{counter, describe_counter, describe_histogram, histogram};
use tracing::Span;

use super::library_path::LibraryPathKind;
use crate::ports::CliError;

/// Counts forwarded invocations by their source and outcome.
pub const FORWARD_TOTAL: &str = "whitaker.forward.total";
/// Records how long a forwarded invocation took.
pub const FORWARD_SECONDS: &str = "whitaker.forward.duration_seconds";
/// Counts library-path resolutions by the outcome that produced them.
pub const LIBRARY_PATH_TOTAL: &str = "whitaker.library_path.total";
/// Records how many candidate directories a resolution contributed.
pub const LIBRARY_PATH_CANDIDATES: &str = "whitaker.library_path.candidates";
/// Counts installation-record reads by whether they succeeded.
pub const INSTALLATION_RECORD_TOTAL: &str = "whitaker.installation_record.total";

/// Where a forwarded invocation ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardSource {
    /// Through the installed bootstrap, which supplies the Clippy snapshot.
    Bootstrap,
    /// Straight to `cargo dylint`, for a machine with no bootstrap.
    Direct,
}

impl ForwardSource {
    /// The bounded label recorded for this source.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::telemetry::ForwardSource;
    ///
    /// assert_eq!(ForwardSource::Bootstrap.label(), "bootstrap");
    /// assert_eq!(ForwardSource::Direct.label(), "direct");
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::Direct => "direct",
        }
    }
}

/// How a forwarded invocation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardOutcome {
    /// The child exited successfully.
    Succeeded,
    /// The child ran and reported a failure of its own.
    ChildFailed,
    /// The child was terminated by a signal.
    Signalled,
    /// The child could not be started.
    DispatchFailed,
    /// The library path could not be resolved, so nothing was started.
    LibraryPathFailed,
}

impl ForwardOutcome {
    /// The bounded label recorded for this outcome.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::telemetry::ForwardOutcome;
    ///
    /// assert_eq!(ForwardOutcome::Succeeded.label(), "succeeded");
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::ChildFailed => "child_failed",
            Self::Signalled => "signalled",
            Self::DispatchFailed => "dispatch_failed",
            Self::LibraryPathFailed => "library_path_failed",
        }
    }

    /// Classify a child's numeric exit status.
    ///
    /// The bootstrap entry point reports only a code, so a signal is inferred
    /// from the shell convention of `128 + signal`. A child that genuinely
    /// exits with such a code is indistinguishable here; the direct path
    /// could tell them apart, but one classification keeps the two sources
    /// comparable.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::telemetry::ForwardOutcome;
    ///
    /// assert_eq!(ForwardOutcome::classify_exit(0), ForwardOutcome::Succeeded);
    /// assert_eq!(
    ///     ForwardOutcome::classify_exit(3),
    ///     ForwardOutcome::ChildFailed
    /// );
    /// assert_eq!(
    ///     ForwardOutcome::classify_exit(130),
    ///     ForwardOutcome::Signalled
    /// );
    /// ```
    #[must_use]
    pub const fn classify_exit(code: i32) -> Self {
        if code == 0 {
            Self::Succeeded
        } else if code >= 128 {
            Self::Signalled
        } else {
            Self::ChildFailed
        }
    }
}

/// Describe every metric this module emits to the active recorder.
///
/// Descriptions belong to the active recorder, so they are registered on each
/// record call rather than through a process-wide guard that could bind them
/// to a no-op or test-local recorder.
fn describe_metrics() {
    describe_counter!(
        FORWARD_TOTAL,
        "Forwarded Dylint invocations, by source and bounded outcome"
    );
    describe_histogram!(
        FORWARD_SECONDS,
        metrics::Unit::Seconds,
        "Elapsed time of forwarded Dylint invocations"
    );
    describe_counter!(
        LIBRARY_PATH_TOTAL,
        "Library-path resolutions, by the outcome that produced them"
    );
    describe_histogram!(
        LIBRARY_PATH_CANDIDATES,
        "Candidate directories contributed to a resolved library path"
    );
    describe_counter!(
        INSTALLATION_RECORD_TOTAL,
        "Installation-record reads, by whether they succeeded"
    );
}

/// Open the span that covers one forwarded invocation.
///
/// # Examples
///
/// ```
/// use whitaker::adapters::{
///     library_path::LibraryPathKind,
///     telemetry::{ForwardSource, forward_span},
/// };
///
/// let span = forward_span(ForwardSource::Direct, LibraryPathKind::NoCandidate);
/// assert_eq!(
///     span.metadata().map(|meta| meta.name()),
///     Some("whitaker.forward")
/// );
/// ```
#[must_use]
pub fn forward_span(source: ForwardSource, kind: LibraryPathKind) -> Span {
    tracing::info_span!(
        "whitaker.forward",
        operation = "forward",
        source = source.label(),
        library_path = kind.label()
    )
}

/// Record the outcome and duration of one forwarded invocation.
pub fn record_forward(source: ForwardSource, outcome: ForwardOutcome, elapsed: Duration) {
    describe_metrics();
    counter!(FORWARD_TOTAL, "source" => source.label(), "outcome" => outcome.label()).increment(1);
    histogram!(FORWARD_SECONDS, "source" => source.label()).record(elapsed.as_secs_f64());
}

/// Record one library-path resolution and how many directories it contributed.
pub fn record_library_path(kind: LibraryPathKind, candidates: usize) {
    describe_metrics();
    // A `usize` to `f64` cast can lose precision above 2^53; a machine cannot
    // contribute that many directories, so the conversion is saturating and
    // explicit rather than lossy and implicit.
    let sample = f64::from(u32::try_from(candidates).unwrap_or(u32::MAX));
    counter!(LIBRARY_PATH_TOTAL, "kind" => kind.label()).increment(1);
    histogram!(LIBRARY_PATH_CANDIDATES, "kind" => kind.label()).record(sample);
}

/// Record whether reading the installation record succeeded.
pub fn record_installation_record_read(succeeded: bool) {
    describe_metrics();
    let outcome = if succeeded { "ok" } else { "failed" };
    counter!(INSTALLATION_RECORD_TOTAL, "operation" => "read", "outcome" => outcome).increment(1);
}

/// Classify a command error without exposing its message.
///
/// The rendered message can name a directory or quote installer detail, so it
/// is useless as a label: the cardinality would follow the user's filesystem.
/// The variant, by contrast, is a closed set.
///
/// # Examples
///
/// ```
/// use whitaker::{adapters::telemetry::error_category, ports::CliError};
///
/// let error = CliError::Inventory {
///     message: "/home/someone/.local/share/whitaker is unavailable".to_owned(),
/// };
/// assert_eq!(error_category(&error), "inventory");
/// ```
#[must_use]
pub const fn error_category(error: &CliError) -> &'static str {
    match error {
        CliError::Install { .. } => "install",
        CliError::Inventory { .. } => "inventory",
        CliError::Forward { .. } => "dispatch",
        CliError::Output { .. } => "output",
        CliError::InstallationRecord { .. } => "installation_record",
        CliError::PrebuiltLibraryPath { .. } => "prebuilt_library_path",
        CliError::LibraryPath { .. } => "library_path",
    }
}

#[cfg(test)]
mod tests;
