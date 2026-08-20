//! Packaging binary for prebuilt lint library distribution.
//!
//! Thin CLI wrapper around [`whitaker_installer::artefact::packaging`]
//! invoked by both the Makefile `package-lints` target and the
//! rolling-release CI workflow.

use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use clap::Parser;
use thiserror::Error;
use whitaker_installer::{
    artefact::{
        error::ArtefactError,
        git_sha::GitSha,
        manifest::GeneratedAt,
        packaging::{PackageParams, generate_manifest_json, package_artefact},
        packaging_error::PackagingError,
        target::TargetTriple,
        toolchain_channel::ToolchainChannel,
    },
    output::write_stderr_line,
    resolution::{LINT_CRATES, SUITE_CRATE},
};

/// Package prebuilt lint libraries into `.tar.zst` archives following
/// the ADR-001 naming convention and write a sidecar
/// `manifest-<target>.json`.
#[derive(Parser, Debug)]
#[command(name = "whitaker-package-lints", version)]
#[command(about = "Package prebuilt lint libraries into .tar.zst archives")]
struct PackageCli {
    /// Git commit SHA (7–40 lowercase hex characters).
    #[arg(long)]
    git_sha: String,

    /// Rust toolchain channel (e.g. "nightly-2026-05-28").
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

    /// Directory containing compiled release libraries. Discovers
    /// files automatically from the canonical crate list.
    #[arg(long, conflicts_with = "library_files")]
    release_dir: Option<PathBuf>,

    /// Paths to compiled library files (required unless --release-dir).
    #[arg(required_unless_present = "release_dir")]
    library_files: Vec<PathBuf>,
}

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

    /// The `--generated-at` value is not valid ISO 8601 (`YYYY-MM-DDThh:mm:ssZ`).
    #[error("invalid --generated-at timestamp: {0}")]
    InvalidTimestamp(String),

    /// The `--release-dir` path is not a directory.
    #[error("--release-dir is not a directory: {0}")]
    NotADirectory(PathBuf),

    /// Failed to read the system clock.
    #[error("system time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),

    /// Failed to write the success report to standard output.
    #[error("failed to write to stdout: {0}")]
    Stdout(#[from] std::io::Error),
}

fn main() {
    let cli = PackageCli::parse();
    if let Err(err) = run(cli) {
        write_stderr_line(&mut std::io::stderr(), format!("error: {err}"));
        std::process::exit(1);
    }
}

/// Validate CLI inputs, delegate to the packaging pipeline, and report
/// the resulting archive path on stdout.
fn run(cli: PackageCli) -> Result<(), PackageCliError> {
    let git_sha = GitSha::try_from(cli.git_sha.as_str())?;
    let toolchain = ToolchainChannel::try_from(cli.toolchain.as_str())?;
    let target = TargetTriple::try_from(cli.target.as_str())?;

    let library_files = if let Some(ref dir) = cli.release_dir {
        if !dir.is_dir() {
            return Err(PackageCliError::NotADirectory(dir.clone()));
        }
        discover_library_files(dir, &target)?
    } else {
        validate_library_files(&cli.library_files)?;
        cli.library_files
    };

    let timestamp = match cli.generated_at {
        Some(ts) => {
            validate_iso8601(&ts)?;
            ts
        }
        None => now_utc_iso8601()?,
    };

    std::fs::create_dir_all(&cli.output_dir).map_err(PackagingError::from)?;

    let out_dir = cli.output_dir.clone();
    let params = PackageParams {
        git_sha,
        toolchain,
        target,
        library_files,
        output_dir: cli.output_dir,
        generated_at: GeneratedAt::new(timestamp),
    };

    let output = package_artefact(&params)?;
    let manifest_json = generate_manifest_json(&output.manifest)?;
    let manifest_filename = format!("manifest-{}.json", output.manifest.target());
    let manifest_path = out_dir.join(manifest_filename);
    std::fs::write(&manifest_path, &manifest_json).map_err(PackagingError::from)?;
    let mut stdout = std::io::stdout();
    writeln!(stdout, "Created {}", output.archive_path.display())?;
    writeln!(stdout, "Manifest {}", manifest_path.display())?;
    Ok(())
}

/// Discover library files in `release_dir` using the canonical crate
/// list.  Fails if any expected library is missing or not a regular file.
fn discover_library_files(
    release_dir: &Path,
    target: &TargetTriple,
) -> Result<Vec<PathBuf>, PackageCliError> {
    let (prefix, ext) = (target.library_prefix(), target.library_extension());
    LINT_CRATES
        .iter()
        .copied()
        .chain(std::iter::once(SUITE_CRATE))
        .map(|name| {
            let filename = format!("{prefix}{name}{ext}");
            let path = release_dir.join(&filename);
            if path.is_file() {
                Ok(path)
            } else {
                Err(PackageCliError::FileNotFound(path))
            }
        })
        .collect()
}

/// Verify that every library file exists on disk and is a regular file.
fn validate_library_files(paths: &[PathBuf]) -> Result<(), PackageCliError> {
    for path in paths {
        if !path.is_file() {
            return Err(PackageCliError::FileNotFound(path.clone()));
        }
    }
    Ok(())
}

/// Verify that `ts` matches the expected `YYYY-MM-DDThh:mm:ssZ` shape.
fn validate_iso8601(ts: &str) -> Result<(), PackageCliError> {
    let &[
        y0,
        y1,
        y2,
        y3,
        b'-',
        mo0,
        mo1,
        b'-',
        d0,
        d1,
        b'T',
        h0,
        h1,
        b':',
        mi0,
        mi1,
        b':',
        s0,
        s1,
        b'Z',
    ] = ts.as_bytes()
    else {
        return Err(PackageCliError::InvalidTimestamp(ts.to_owned()));
    };
    let digits = [y0, y1, y2, y3, mo0, mo1, d0, d1, h0, h1, mi0, mi1, s0, s1];
    if digits.iter().all(u8::is_ascii_digit) {
        Ok(())
    } else {
        Err(PackageCliError::InvalidTimestamp(ts.to_owned()))
    }
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
    let day_secs = epoch_secs.rem_euclid(86_400);
    let hour = day_secs.div_euclid(3_600);
    let minute = day_secs.rem_euclid(3_600).div_euclid(60);
    let second = day_secs.rem_euclid(60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Convert a Unix epoch timestamp to a `(year, month, day)` triple.
///
/// Adapted from Howard Hinnant's `civil_from_days` algorithm, which is
/// public domain and widely used in C++ `<chrono>` implementations.
const fn civil_from_epoch(epoch_secs: u64) -> (i64, u64, u64) {
    let z = epoch_secs.div_euclid(86_400).cast_signed() + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097).cast_unsigned(); // day of era [0, 146_096]
    let yoe = (doe - doe.div_euclid(1_460) + doe.div_euclid(36_524) - doe.div_euclid(146_096))
        .div_euclid(365);
    let y = yoe.cast_signed() + era * 400;
    let doy = doe - (365 * yoe + yoe.div_euclid(4) - yoe.div_euclid(100)); // day of year
    let mp = (5 * doy + 2).div_euclid(153);
    let d = doy - (153 * mp + 2).div_euclid(5) + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use clap::Parser;
    use rstest::{fixture, rstest};

    use super::*;

    /// Common CLI base arguments shared across parsing tests.
    const BASE_ARGS: [&str; 9] = [
        "whitaker-package-lints",
        "--git-sha",
        "abc1234",
        "--toolchain",
        "nightly-2026-05-28",
        "--target",
        "x86_64-unknown-linux-gnu",
        "--output-dir",
        "/tmp/dist",
    ];

    /// Build a CLI arg vec from base args plus extra trailing args.
    fn cli_args<'a>(extra: &'a [&'a str]) -> Vec<&'a str> {
        BASE_ARGS
            .iter()
            .copied()
            .chain(extra.iter().copied())
            .collect()
    }

    #[fixture]
    fn linux_target() -> TargetTriple {
        TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid")
    }

    #[test]
    fn cli_parses_all_required_args() {
        let cli = PackageCli::parse_from(cli_args(&["/tmp/libfoo.so", "/tmp/libbar.so"]));
        assert_eq!(cli.git_sha, "abc1234");
        assert_eq!(cli.toolchain, "nightly-2026-05-28");
        assert_eq!(cli.target, "x86_64-unknown-linux-gnu");
        assert_eq!(cli.output_dir, PathBuf::from("/tmp/dist"));
        assert_eq!(cli.library_files.len(), 2);
        assert!(cli.generated_at.is_none());
    }

    #[test]
    fn cli_accepts_optional_generated_at() {
        let args = cli_args(&["--generated-at", "2026-02-12T10:00:00Z", "/tmp/lib.so"]);
        let cli = PackageCli::parse_from(args);
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
    fn cli_rejects_missing_library_files_and_release_dir() {
        PackageCli::try_parse_from(cli_args(&[]))
            .expect_err("expected clap to reject zero library files");
    }

    #[test]
    fn cli_parses_release_dir_flag() {
        let cli = PackageCli::parse_from(cli_args(&["--release-dir", "/tmp/target/release"]));
        assert_eq!(cli.release_dir, Some(PathBuf::from("/tmp/target/release")));
        assert!(cli.library_files.is_empty());
    }

    #[test]
    fn cli_rejects_both_release_dir_and_library_files() {
        PackageCli::try_parse_from(cli_args(&["--release-dir", "/tmp/release", "/tmp/lib.so"]))
            .expect_err("expected clap to reject conflicting args");
    }

    #[rstest]
    fn discover_library_files_finds_expected_files(linux_target: TargetTriple) {
        let dir = tempfile::tempdir().expect("temp dir");
        for name in LINT_CRATES.iter().chain(std::iter::once(&SUITE_CRATE)) {
            fs::write(dir.path().join(format!("lib{name}.so")), b"fake").expect("write");
        }
        let found = discover_library_files(dir.path(), &linux_target).expect("all present");
        assert_eq!(found.len(), LINT_CRATES.len() + 1);
    }

    #[test]
    fn discover_library_files_uses_correct_extension() {
        let target = TargetTriple::try_from("aarch64-apple-darwin").expect("valid");
        let dir = tempfile::tempdir().expect("temp dir");
        for name in LINT_CRATES.iter().chain(std::iter::once(&SUITE_CRATE)) {
            fs::write(dir.path().join(format!("lib{name}.dylib")), b"fake").expect("write");
        }
        let found = discover_library_files(dir.path(), &target).expect("all present");
        assert!(
            found
                .iter()
                .all(|p| p.to_string_lossy().ends_with(".dylib"))
        );
    }

    #[rstest]
    fn discover_library_files_rejects_missing(linux_target: TargetTriple) {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("libconditional_max_n_branches.so"), b"fake").expect("write");
        let result = discover_library_files(dir.path(), &linux_target);
        assert!(result.is_err(), "must reject incomplete set of libraries");
    }

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
        assert!(validate_iso8601(&ts).is_ok(), "own output must validate");
    }

    #[rstest]
    #[case::valid("2026-02-12T10:00:00Z", true)]
    #[case::too_short("2026-02-12T10:00Z", false)]
    #[case::no_z("2026-02-12T10:00:00X", false)]
    #[case::letters("XXXX-XX-XXTXX:XX:XXZ", false)]
    fn validate_iso8601_accepts_and_rejects(#[case] ts: &str, #[case] ok: bool) {
        assert_eq!(validate_iso8601(ts).is_ok(), ok);
    }

    #[test]
    fn release_dir_rejects_non_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("not-a-dir");
        fs::write(&file, b"x").expect("write");
        let file_str = file.to_str().expect("utf8").to_owned();
        let extra = ["--release-dir", &file_str];
        let cli = PackageCli::parse_from(cli_args(&extra));
        assert!(
            run(cli).is_err(),
            "should reject non-directory --release-dir"
        );
    }
}
