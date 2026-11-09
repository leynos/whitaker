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
use whitaker_installer::installation_record::RecordContents;

use super::library_path::LibraryPathKind;
use crate::ports::CliError;

/// Counts forwarded invocations by their source and outcome.
pub const FORWARD_TOTAL: &str = "whitaker.forward.total";
/// Records how long a forwarded invocation took.
pub const FORWARD_SECONDS: &str = "whitaker.forward.duration_seconds";
/// Counts library-path resolutions by the outcome that produced them.
pub const LIBRARY_PATH_TOTAL: &str = "whitaker.library_path.total";
/// Records how long a library-path resolution took.
pub const LIBRARY_PATH_SECONDS: &str = "whitaker.library_path.duration_seconds";
/// Records how many candidate directories a resolution contributed.
pub const LIBRARY_PATH_CANDIDATES: &str = "whitaker.library_path.candidates";
/// Counts candidate directories dropped during a resolution, by reason.
pub const LIBRARY_PATH_DROPPED: &str = "whitaker.library_path.dropped";
/// Counts installation-record operations by operation and outcome.
pub const INSTALLATION_RECORD_TOTAL: &str = "whitaker.installation_record.total";
/// Counts writes of CLI diagnostics by stream and outcome.
pub const DIAGNOSTIC_TOTAL: &str = "whitaker.diagnostic.total";

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
    /// from the shell convention of `128 + signal`. That convention only
    /// covers the signals a POSIX shell can report, 1 through 64, so the
    /// window is bounded: an unrelated status such as the Windows code 256 is
    /// a failure of its own rather than a signal. A child that genuinely exits
    /// inside the window is still indistinguishable from one killed by a
    /// signal, because a bare code carries no more information than that.
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
    /// assert_eq!(
    ///     ForwardOutcome::classify_exit(256),
    ///     ForwardOutcome::ChildFailed
    /// );
    /// ```
    #[must_use]
    pub const fn classify_exit(code: i32) -> Self {
        if code == 0 {
            Self::Succeeded
        } else if matches!(code, 129..=192) {
            Self::Signalled
        } else {
            Self::ChildFailed
        }
    }
}

/// The bounded outcome of reading the installation record.
///
/// An empty path list cannot distinguish "nothing has been installed" from
/// "the installation has gone away", so the read outcome names which of those
/// happened. Every label is drawn from this closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordReadOutcome {
    /// No record exists, so nothing has been installed yet.
    Missing,
    /// The record exists but stores no staging paths.
    Empty,
    /// The record stores paths, but none of them still exist.
    Stale,
    /// At least one recorded staging directory survives.
    Current,
    /// The record stores paths, none still exists, and at least one could not be
    /// inspected.
    Uninspectable,
    /// The record exists but could not be locked, read, or parsed.
    Failed,
}

impl RecordReadOutcome {
    /// The bounded label recorded for this outcome.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::telemetry::RecordReadOutcome;
    ///
    /// assert_eq!(RecordReadOutcome::Stale.label(), "stale");
    /// assert_eq!(RecordReadOutcome::Uninspectable.label(), "uninspectable");
    /// assert_eq!(RecordReadOutcome::Failed.label(), "failed");
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Empty => "empty",
            Self::Stale => "stale",
            Self::Current => "current",
            Self::Uninspectable => "uninspectable",
            Self::Failed => "failed",
        }
    }
}

impl From<RecordContents> for RecordReadOutcome {
    fn from(contents: RecordContents) -> Self {
        match contents {
            RecordContents::Missing => Self::Missing,
            RecordContents::Empty => Self::Empty,
            RecordContents::Stale => Self::Stale,
            RecordContents::Current => Self::Current,
            RecordContents::Uninspectable => Self::Uninspectable,
        }
    }
}

/// The stream a CLI diagnostic was written to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStream {
    /// Standard output, where clap writes help and version text.
    Stdout,
    /// Standard error, where errors and progress go.
    Stderr,
}

impl DiagnosticStream {
    /// The bounded label recorded for this stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::telemetry::DiagnosticStream;
    ///
    /// assert_eq!(DiagnosticStream::Stderr.label(), "stderr");
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
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
        LIBRARY_PATH_SECONDS,
        metrics::Unit::Seconds,
        "Elapsed time of library-path resolutions"
    );
    describe_histogram!(
        LIBRARY_PATH_CANDIDATES,
        "Candidate directories contributed to a resolved library path"
    );
    describe_counter!(
        LIBRARY_PATH_DROPPED,
        "Candidate directories dropped during resolution, by reason"
    );
    describe_counter!(
        INSTALLATION_RECORD_TOTAL,
        "Installation-record operations, by operation and outcome"
    );
    describe_counter!(
        DIAGNOSTIC_TOTAL,
        "CLI diagnostics, by stream and whether writing them succeeded"
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

/// Record one library-path resolution, how many directories it contributed,
/// and how long it took.
///
/// Resolution stats the filesystem once per candidate, so its duration is
/// recorded separately from the forward it feeds: a slow resolution and a slow
/// child would otherwise be indistinguishable in the forwarding histogram.
pub fn record_library_path(kind: LibraryPathKind, candidates: usize, elapsed: Duration) {
    describe_metrics();
    // A `usize` to `f64` cast can lose precision above 2^53; a machine cannot
    // contribute that many directories, so the conversion is saturating and
    // explicit rather than lossy and implicit.
    let sample = f64::from(u32::try_from(candidates).unwrap_or(u32::MAX));
    counter!(LIBRARY_PATH_TOTAL, "kind" => kind.label()).increment(1);
    histogram!(LIBRARY_PATH_SECONDS, "kind" => kind.label()).record(elapsed.as_secs_f64());
    histogram!(LIBRARY_PATH_CANDIDATES, "kind" => kind.label()).record(sample);
}

/// Record how many candidate directories a resolution dropped, by reason.
///
/// `absent` counts candidates that no longer exist, which is the normal result
/// of an earlier install being removed. `unusable` counts candidates that do
/// exist but could not be inspected or represented, which is the signal that a
/// data directory is broken rather than stale.
pub fn record_dropped_candidates(absent: usize, unusable: usize) {
    describe_metrics();
    counter!(LIBRARY_PATH_DROPPED, "reason" => "absent")
        .increment(u64::try_from(absent).unwrap_or(u64::MAX));
    counter!(LIBRARY_PATH_DROPPED, "reason" => "unusable")
        .increment(u64::try_from(unusable).unwrap_or(u64::MAX));
}

/// Record how reading the installation record turned out.
pub fn record_installation_record_read(outcome: RecordReadOutcome) {
    describe_metrics();
    counter!(INSTALLATION_RECORD_TOTAL, "operation" => "read", "outcome" => outcome.label())
        .increment(1);
}

/// Record whether one CLI diagnostic reached its stream.
///
/// A failed diagnostic write cannot change an outcome that has already been
/// decided, so it is recorded here instead of being folded into the exit
/// status the caller sees.
pub fn record_diagnostic_write(stream: DiagnosticStream, succeeded: bool) {
    describe_metrics();
    let outcome = if succeeded { "ok" } else { "failed" };
    counter!(DIAGNOSTIC_TOTAL, "stream" => stream.label(), "outcome" => outcome).increment(1);
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
        CliError::ToolchainDetection { .. } => "toolchain_detection",
        CliError::PrebuiltLibraryPath { .. } => "prebuilt_library_path",
        CliError::LibraryPathUnusable { .. } => "library_path_unusable",
        CliError::LibraryPath { .. } => "library_path",
    }
}

#[cfg(test)]
mod tests;
