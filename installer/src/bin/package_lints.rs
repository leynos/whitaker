//! Packaging binary for prebuilt lint library distribution.
//!
//! Thin CLI wrapper around [`whitaker_installer::artefact::packaging`] that
//! the Makefile `package-lints` target and the rolling-release CI workflow
//! both invoke.  Centralising the archive creation and manifest emission in
//! Rust eliminates the risk of drift between shell reimplementations.

use clap::Parser;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use whitaker_installer::artefact::error::ArtefactError;
use whitaker_installer::artefact::git_sha::GitSha;
use whitaker_installer::artefact::manifest::GeneratedAt;
use whitaker_installer::artefact::packaging::{PackageParams, package_artefact};
use whitaker_installer::artefact::packaging_error::PackagingError;
use whitaker_installer::artefact::target::TargetTriple;
use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// Package prebuilt lint libraries into `.tar.zst` archives.
///
/// Creates a single archive following the ADR-001 naming convention
/// (`whitaker-lints-<sha>-<toolchain>-<target>.tar.zst`) with an embedded
/// `manifest.json` containing git SHA, toolchain, target, file list, and a
/// SHA-256 digest of the archive itself.
#[derive(Parser, Debug)]
#[command(name = "whitaker-package-lints")]
#[command(
    version,
    about = "Package prebuilt lint libraries into .tar.zst archives"
)]
struct PackageCli {
    /// Git commit SHA (7–40 lowercase hex characters).
    #[arg(long)]
    git_sha: String,

    /// Rust toolchain channel (e.g. "nightly-2025-09-18").
    #[arg(long)]
    toolchain: String,

    /// Target triple (e.g. "x86_64-unknown-linux-gnu").
    #[arg(long)]
    target: String,

    /// Directory where the output archive will be written.
    #[arg(long)]
    output_dir: PathBuf,

    /// ISO 8601 timestamp for the build [default: current UTC time].
    #[arg(long)]
    generated_at: Option<String>,

    /// Paths to the compiled library files to include in the archive.
    #[arg(required = true)]
    library_files: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by the packaging CLI.
#[derive(Debug, Error)]
enum PackageCliError {
    /// An artefact domain validation error (invalid SHA, target, etc.).
    #[error("{0}")]
    Artefact(#[from] ArtefactError),

    /// An error during archive creation or manifest emission.
    #[error("{0}")]
    Packaging(#[from] PackagingError),

    /// A library file supplied on the command line does not exist.
    #[error("library file not found: {0}")]
    FileNotFound(PathBuf),

    /// Failed to read the system clock.
    #[error("system time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

fn main() {
    let cli = PackageCli::parse();
    if let Err(err) = run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

/// Validate CLI inputs, delegate to the packaging pipeline, and report
/// the resulting archive path on stdout.
fn run(cli: PackageCli) -> Result<(), PackageCliError> {
    let git_sha = GitSha::try_from(cli.git_sha.as_str())?;
    let toolchain = ToolchainChannel::try_from(cli.toolchain.as_str())?;
    let target = TargetTriple::try_from(cli.target.as_str())?;

    validate_library_files(&cli.library_files)?;

    let timestamp = match cli.generated_at {
        Some(ts) => ts,
        None => now_utc_iso8601()?,
    };

    let params = PackageParams {
        git_sha,
        toolchain,
        target,
        library_files: cli.library_files,
        output_dir: cli.output_dir,
        generated_at: GeneratedAt::new(timestamp),
    };

    let output = package_artefact(params)?;

    println!("Created {}", output.archive_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Verify that every library file exists on disk.
fn validate_library_files(paths: &[PathBuf]) -> Result<(), PackageCliError> {
    for path in paths {
        if !path_exists(path) {
            return Err(PackageCliError::FileNotFound(path.clone()));
        }
    }
    Ok(())
}

/// Thin wrapper to allow testing without touching the filesystem.
fn path_exists(path: &Path) -> bool {
    path.exists()
}

/// Return the current UTC time as an ISO 8601 string (`YYYY-MM-DDThh:mm:ssZ`).
///
/// Uses `std::time::SystemTime` to avoid pulling in `chrono`.
fn now_utc_iso8601() -> Result<String, std::time::SystemTimeError> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(format_epoch_secs(secs))
}

/// Format a Unix epoch timestamp as `YYYY-MM-DDThh:mm:ssZ`.
fn format_epoch_secs(epoch_secs: u64) -> String {
    let (year, month, day) = civil_from_epoch(epoch_secs);
    let day_secs = (epoch_secs % 86_400) as u32;
    let hour = day_secs / 3_600;
    let minute = (day_secs % 3_600) / 60;
    let second = day_secs % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Convert a Unix epoch timestamp to a `(year, month, day)` triple.
///
/// Adapted from Howard Hinnant's `civil_from_days` algorithm, which is
/// public domain and widely used in C++ `<chrono>` implementations.
fn civil_from_epoch(epoch_secs: u64) -> (u32, u32, u32) {
    let z = (epoch_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64; // day of era [0, 146_096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    #[expect(
        clippy::cast_sign_loss,
        reason = "year is always positive for post-epoch dates"
    )]
    (y as u32, m as u32, d as u32)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::rstest;

    #[test]
    fn cli_parses_all_required_args() {
        let cli = PackageCli::parse_from([
            "whitaker-package-lints",
            "--git-sha",
            "abc1234",
            "--toolchain",
            "nightly-2025-09-18",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--output-dir",
            "/tmp/dist",
            "/tmp/libfoo.so",
            "/tmp/libbar.so",
        ]);
        assert_eq!(cli.git_sha, "abc1234");
        assert_eq!(cli.toolchain, "nightly-2025-09-18");
        assert_eq!(cli.target, "x86_64-unknown-linux-gnu");
        assert_eq!(cli.output_dir, PathBuf::from("/tmp/dist"));
        assert_eq!(cli.library_files.len(), 2);
        assert!(cli.generated_at.is_none());
    }

    #[test]
    fn cli_accepts_optional_generated_at() {
        let cli = PackageCli::parse_from([
            "whitaker-package-lints",
            "--git-sha",
            "abc1234",
            "--toolchain",
            "nightly-2025-09-18",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--output-dir",
            "/tmp/dist",
            "--generated-at",
            "2026-02-12T10:00:00Z",
            "/tmp/lib.so",
        ]);
        assert_eq!(cli.generated_at, Some("2026-02-12T10:00:00Z".to_owned()));
    }

    #[test]
    fn cli_rejects_missing_git_sha() {
        PackageCli::try_parse_from([
            "whitaker-package-lints",
            "--toolchain",
            "nightly",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--output-dir",
            "/tmp",
            "/tmp/lib.so",
        ])
        .expect_err("expected clap to reject missing --git-sha");
    }

    #[test]
    fn cli_rejects_missing_library_files() {
        PackageCli::try_parse_from([
            "whitaker-package-lints",
            "--git-sha",
            "abc1234",
            "--toolchain",
            "nightly",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--output-dir",
            "/tmp",
        ])
        .expect_err("expected clap to reject zero library files");
    }

    /// Known epoch values for timestamp formatting validation.
    #[rstest]
    #[case::unix_epoch(0, "1970-01-01T00:00:00Z")]
    #[case::y2k(946_684_800, "2000-01-01T00:00:00Z")]
    #[case::midday_2026(1_771_156_800, "2026-02-15T12:00:00Z")]
    fn format_epoch_secs_produces_correct_iso8601(#[case] secs: u64, #[case] expected: &str) {
        assert_eq!(format_epoch_secs(secs), expected);
    }

    #[test]
    fn now_utc_iso8601_format_is_valid() {
        let ts = now_utc_iso8601().expect("system time");
        assert_eq!(ts.len(), 20, "ISO 8601 timestamp must be 20 characters");
        assert!(ts.ends_with('Z'), "must end with Z");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }
}
