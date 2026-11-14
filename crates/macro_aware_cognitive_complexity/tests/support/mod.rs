//! Shared plumbing for the macro-aware cognitive-complexity integration tests.
//!
//! The parity check builds this lint crate and runs pinned Clippy and this lint
//! over one generated fixture, so these helpers own nested Cargo invocation,
//! fixture staging, and diagnostic parsing. Later feature-matrix targets reuse
//! them, and the fixture source lives in `tests/data` as data.

use std::{
    env,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cargo_metadata::Message;
use serde_json::Value;
use tempfile::TempDir;

/// The lint package under test.
pub(super) const LINT_PACKAGE: &str = "macro_aware_cognitive_complexity";
/// The Clippy lint this crate reimplements.
pub(super) const CLIPPY_CODE: &str = "clippy::cognitive_complexity";
/// The diagnostic code this crate emits.
pub(super) const LINT_CODE: &str = "macro_aware_cognitive_complexity";
/// The phrase both tools place immediately before the reported score.
pub(super) const SCORE_MARKER: &str = "cognitive complexity of ";

// Fixture inputs are pinned byte-for-byte because `EXPECTED_SCORES` anchors on
// their exact line numbers.
pub(super) const FIXTURE_MANIFEST: &str = include_str!("../data/fixture_manifest.toml");
pub(super) const FIXTURE_CLIPPY_CONFIG: &str = include_str!("../data/fixture_clippy.toml");
pub(super) const FIXTURE_DYLINT_CONFIG: &str = include_str!("../data/fixture_dylint.toml");
pub(super) const FIXTURE_SOURCE: &str = include_str!("../data/fixture_lib.rs");

/// Captured output of one lint invocation.
pub(super) struct RawRun {
    /// Whether the command exited successfully.
    pub(super) is_success: bool,
    /// The raw JSON message stream.
    pub(super) stdout: Vec<u8>,
    /// The rendered standard-error text.
    pub(super) stderr: String,
}

impl RawRun {
    /// Capture one finished command's status and streams.
    pub(super) fn from_output(output: Output) -> Self {
        Self {
            is_success: output.status.success(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// Run `args` through Cargo, advising installation when the tool is missing.
pub(super) fn ensure_tool(description: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("cargo")
        .args(args)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain()?)
        .output()
        .with_context(|| format!("failed to run `{description}`; is it installed?"))?;
    if !output.status.success() {
        anyhow::bail!(
            "`{description}` failed; install Clippy with `rustup component add clippy` and Dylint \
             with `cargo install cargo-dylint dylint-link`; stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Find the workspace root containing this crate's manifest.
pub(super) fn workspace_root() -> anyhow::Result<Utf8PathBuf> {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidate = manifest_dir.as_path();
    loop {
        if is_workspace_root(candidate)? {
            return Ok(candidate.to_path_buf());
        }
        candidate = candidate
            .parent()
            .context("workspace root not found above CARGO_MANIFEST_DIR")?;
    }
}

/// Check whether `candidate` contains a workspace manifest.
///
/// The result is `false` only when the manifest is absent or not a workspace;
/// an unreadable directory or manifest is an error.
pub(super) fn is_workspace_root(candidate: &Utf8Path) -> anyhow::Result<bool> {
    let directory = Dir::open_ambient_dir(candidate, ambient_authority())
        .with_context(|| format!("failed to open candidate directory `{candidate}`"))?;
    match directory.read_to_string("Cargo.toml") {
        Ok(manifest) => Ok(manifest.contains("[workspace]")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error)
            .with_context(|| format!("failed to read workspace manifest in `{candidate}`")),
    }
}

/// Create an isolated target directory under the workspace's `target/tests`.
///
/// Neither `/tmp` nor the workspace's own `debug` directory is shared with a
/// staged lint library.
pub(super) fn private_target_dir(
    workspace_root: &Utf8Path,
    label: &str,
) -> anyhow::Result<TempDir> {
    let workspace = Dir::open_ambient_dir(workspace_root, ambient_authority())
        .context("failed to open the workspace for an isolated target directory")?;
    workspace
        .create_dir_all("target/tests/parity")
        .context("failed to create the isolated target parent directory")?;
    TempDir::new_in(workspace_root.join("target/tests/parity").as_std_path())
        .with_context(|| format!("failed to create the isolated `{label}` target directory"))
}

/// Build the lint library and stage a toolchain-qualified copy for Dylint.
pub(super) fn build_and_stage_library(
    workspace_root: &Utf8Path,
    target_dir: &Utf8Path,
) -> anyhow::Result<Utf8PathBuf> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--lib",
            "--quiet",
            "--message-format=json",
            "-p",
            LINT_PACKAGE,
            "--features",
            "dylint-driver",
        ])
        .current_dir(workspace_root.as_std_path())
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .context("failed to execute cargo build for the lint library")?;
    if !output.status.success() {
        anyhow::bail!(
            "lint library build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let cdylib_path = find_cdylib_in_artefacts(&output.stdout)?;
    let debug_dir = cdylib_path
        .parent()
        .context("staged cdylib has no parent directory")?
        .to_owned();
    stage_toolchain_qualified_library(&cdylib_path, &debug_dir)?;
    Ok(debug_dir)
}

/// Extract the lint package's cdylib path from cargo build JSON output.
pub(super) fn find_cdylib_in_artefacts(stdout: &[u8]) -> anyhow::Result<Utf8PathBuf> {
    for parsed in Message::parse_stream(Cursor::new(stdout)) {
        let message = parsed.context("failed to parse cargo build JSON output")?;
        let Message::CompilerArtifact(artefact) = message else {
            continue;
        };
        if artefact.target.name != LINT_PACKAGE || !artefact.target.is_cdylib() {
            continue;
        }
        if let Some(path) = artefact
            .filenames
            .iter()
            .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        {
            return Ok(path.clone());
        }
    }
    anyhow::bail!("cdylib artefact for `{LINT_PACKAGE}` not found in build output")
}

/// Copy the cdylib to a toolchain-qualified filename for Dylint discovery.
pub(super) fn stage_toolchain_qualified_library(
    cdylib_path: &Utf8Path,
    directory: &Utf8Path,
) -> anyhow::Result<()> {
    let toolchain = pinned_toolchain()?;
    let file_name = cdylib_path
        .file_name()
        .context("staged cdylib has no file name")?;
    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );
    let staged_directory = Dir::open_ambient_dir(directory, ambient_authority())
        .context("failed to open the staged library directory")?;
    staged_directory
        .copy(file_name, &staged_directory, &target_name)
        .with_context(|| format!("failed to stage lint library as `{target_name}`"))?;
    Ok(())
}

/// Return the toolchain the lint library is built with.
///
/// `cargo dylint` is told the same toolchain explicitly, so the fixture
/// directory's own toolchain resolution cannot pick a different nightly.
///
/// # Errors
///
/// Return an error when neither the run-time nor the compile-time
/// `RUSTUP_TOOLCHAIN` is set, so the pinned toolchain cannot be determined. The
/// test must then run through rustup's Cargo proxy, which sets it from
/// `rust-toolchain.toml`; a sentinel would stage a library under a name Dylint
/// never searches for.
pub(super) fn pinned_toolchain() -> anyhow::Result<String> {
    env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .context(
            "cannot determine the pinned toolchain: neither the run-time nor the compile-time \
             RUSTUP_TOOLCHAIN is set; run this test through rustup's Cargo proxy so it selects \
             the toolchain from `rust-toolchain.toml`",
        )
}

/// Write the fixture project files beneath `root`.
pub(super) fn write_fixture(root: &Utf8Path) -> anyhow::Result<()> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .context("failed to open the fixture root")?;
    directory
        .create_dir_all("src")
        .context("failed to create the fixture `src` directory")?;
    directory.write("Cargo.toml", FIXTURE_MANIFEST)?;
    directory.write("clippy.toml", FIXTURE_CLIPPY_CONFIG)?;
    directory.write("dylint.toml", FIXTURE_DYLINT_CONFIG)?;
    directory.write("src/lib.rs", FIXTURE_SOURCE)?;
    Ok(())
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
pub(super) fn run_clippy(fixture: &Path, target_dir: &Utf8Path) -> anyhow::Result<RawRun> {
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
pub(super) fn run_whitaker(
    fixture: &Path,
    target_dir: &Utf8Path,
    library_dir: &Utf8Path,
) -> anyhow::Result<RawRun> {
    let output = Command::new("cargo")
        .args(["dylint", "--all", "--", "--message-format", "json"])
        .current_dir(fixture)
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
pub(super) fn compiler_messages(raw: &RawRun) -> Vec<Value> {
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
pub(super) fn lint_scores(raw: &RawRun, code: &str) -> anyhow::Result<Vec<(u32, u64)>> {
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
pub(super) fn parse_score(message_text: &str) -> Option<u64> {
    let (_, rest) = message_text.split_once(SCORE_MARKER)?;
    let digits_start = rest.strip_prefix('(').unwrap_or(rest);
    let digit_count = digits_start.bytes().take_while(u8::is_ascii_digit).count();
    digits_start.get(..digit_count)?.parse().ok()
}

/// Find the 1-based line of the first fixture line containing `needle`.
pub(super) fn fixture_line(needle: &str) -> anyhow::Result<u32> {
    let index = FIXTURE_SOURCE
        .lines()
        .position(|line| line.contains(needle))
        .with_context(|| format!("fixture source has no line containing `{needle}`"))?;
    u32::try_from(index + 1).context("fixture line number does not fit in `u32`")
}

/// Return a UTF-8 view of a temporary directory's path.
pub(super) fn utf8_temp_path(directory: &TempDir) -> anyhow::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary path is not UTF-8: {}", path.display()))
}
