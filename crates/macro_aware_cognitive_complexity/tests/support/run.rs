//! Nested `cargo clippy` and `cargo dylint` runs, and diagnostic parsing.
//!
//! The parity check drives both tools over one fixture and compares the scores
//! they report. Both runs capture a JSON message stream rather than relying on
//! exit status, and the parsers below turn that stream into comparable
//! `(anchor line, score)` pairs.

use std::{
    path::Path,
    process::{Command, Output},
};

use anyhow::Context as _;
use camino::Utf8Path;
use serde_json::Value;

use super::cargo::pinned_toolchain;

/// The Clippy lint this crate reimplements.
pub(crate) const CLIPPY_CODE: &str = "clippy::cognitive_complexity";
/// The diagnostic code this crate emits.
pub(crate) const LINT_CODE: &str = "macro_aware_cognitive_complexity";
/// The phrase both tools place immediately before the reported score.
pub(crate) const SCORE_MARKER: &str = "cognitive complexity of ";

/// Captured output of one lint invocation.
pub(crate) struct RawRun {
    /// Whether the command exited successfully.
    pub(crate) is_success: bool,
    /// The raw JSON message stream.
    pub(crate) stdout: Vec<u8>,
    /// The rendered standard-error text.
    pub(crate) stderr: String,
}

impl RawRun {
    /// Capture one finished command's status and streams.
    pub(crate) fn from_output(output: Output) -> Self {
        Self {
            is_success: output.status.success(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// Run pinned Clippy over the fixture and capture its JSON stream.
///
/// `RUSTFLAGS` is removed so the outer gate's `-D warnings` cannot turn the
/// fixture's measured warnings into a build failure. A non-zero Clippy exit is
/// captured rather than propagated.
///
/// # Errors
///
/// Return an error when the pinned toolchain cannot be determined, which
/// happens before Cargo starts, or when Cargo cannot be spawned.
pub(crate) fn run_clippy(fixture: &Path, target_dir: &Utf8Path) -> anyhow::Result<RawRun> {
    let output = Command::new("cargo")
        .args([
            "clippy",
            "--lib",
            "--message-format=json",
            "--",
            "-W",
            CLIPPY_CODE,
        ])
        .current_dir(fixture)
        // Offline: the fixture's dependencies are already in the shared cache.
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain()?)
        .env_remove("RUSTFLAGS")
        .env_remove("DYLINT_LIBRARY_PATH")
        .env_remove("DYLINT_RUSTFLAGS")
        .output()
        .context("failed to execute cargo clippy")?;
    Ok(RawRun::from_output(output))
}

/// Run this lint through `cargo dylint` and capture its JSON stream.
///
/// `DYLINT_RUSTFLAGS=-D warnings` denies each finding, so a diagnostic makes
/// the command fail while the JSON stream stays complete. `DYLINT_LOCALE` is
/// pinned to `en-GB` on the child only: a developer environment set to `cy` or
/// `gd` would otherwise render a translated message that the score parser
/// cannot read, because only the English text carries the score marker.
///
/// # Errors
///
/// Return an error when the pinned toolchain cannot be determined, which
/// happens before Cargo starts, or when Cargo cannot be spawned.
pub(crate) fn run_whitaker(
    fixture: &Path,
    target_dir: &Utf8Path,
    library_dir: &Utf8Path,
) -> anyhow::Result<RawRun> {
    let output = Command::new("cargo")
        .args(["dylint", "--all", "--", "--message-format", "json"])
        .current_dir(fixture)
        // Offline: the fixture's dependencies are already in the shared cache.
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", target_dir)
        .env("DYLINT_LIBRARY_PATH", library_dir)
        .env("DYLINT_RUSTFLAGS", "-D warnings")
        .env("DYLINT_LOCALE", "en-GB")
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain()?)
        .env_remove("RUSTFLAGS")
        .output()
        .context("failed to execute cargo dylint")?;
    Ok(RawRun::from_output(output))
}

/// Return the `compiler-message` values from a captured cargo JSON stream.
///
/// Non-JSON lines are skipped because Cargo may interleave plain text.
pub(crate) fn compiler_messages(raw: &RawRun) -> Vec<Value> {
    raw.stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .filter(|message| message.get("reason").and_then(Value::as_str) == Some("compiler-message"))
        .collect()
}

/// Collect `(anchor line, score)` pairs for diagnostics carrying `code`.
///
/// # Errors
///
/// Return an error when a selected diagnostic has no primary span line or its
/// rendered message does not carry a parseable score.
pub(crate) fn lint_scores(raw: &RawRun, code: &str) -> anyhow::Result<Vec<(u32, u64)>> {
    let mut scores = Vec::new();
    for message in compiler_messages(raw) {
        let diagnostic = message
            .get("message")
            .context("compiler message has no `message` field")?;
        let is_under_test = diagnostic
            .get("code")
            .and_then(|entry| entry.get("code"))
            .and_then(Value::as_str)
            == Some(code);
        if !is_under_test {
            continue;
        }
        let Some(line) = primary_span_line(diagnostic)? else {
            continue;
        };
        let rendered = diagnostic
            .get("message")
            .and_then(Value::as_str)
            .context("diagnostic has no rendered message")?;
        let score = parse_score(rendered).with_context(|| {
            format!("could not parse a cognitive complexity score from `{rendered}`")
        })?;
        scores.push((line, score));
    }
    scores.sort_unstable();
    Ok(scores)
}

/// Return the 1-based line of `diagnostic`'s primary span.
///
/// A diagnostic with no primary span yields `None`, which callers skip rather
/// than scoring at a guessed anchor.
///
/// # Errors
///
/// Return an error when the primary span has no `line_start` or its line does
/// not fit in `u32`.
fn primary_span_line(diagnostic: &Value) -> anyhow::Result<Option<u32>> {
    let Some(span) = diagnostic
        .get("spans")
        .and_then(Value::as_array)
        .and_then(|spans| {
            spans
                .iter()
                .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
        })
    else {
        return Ok(None);
    };
    let line_start = span
        .get("line_start")
        .and_then(Value::as_u64)
        .context("primary span has no `line_start`")?;
    u32::try_from(line_start)
        .context("anchor line does not fit in `u32`")
        .map(Some)
}

/// Read the leading digits after `"cognitive complexity of "`.
///
/// Clippy wraps the score in parentheses (`(N/M)`); Whitaker does not, so an
/// optional opening parenthesis is skipped. Absent digits yield `None` so a
/// message-format change fails loudly rather than silently scoring zero.
pub(crate) fn parse_score(message_text: &str) -> Option<u64> {
    let (_, rest) = message_text.split_once(SCORE_MARKER)?;
    let digits_start = rest.strip_prefix('(').unwrap_or(rest);
    let digit_count = digits_start.bytes().take_while(u8::is_ascii_digit).count();
    digits_start.get(..digit_count)?.parse().ok()
}
