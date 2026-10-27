//! Tests for the lint-library packaging command.

use std::{fs, io};

use clap::Parser;
use proptest::prelude::*;
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
fn linux_target() -> Result<TargetTriple, ArtefactError> {
    TargetTriple::try_from("x86_64-unknown-linux-gnu")
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
fn discover_library_files_finds_expected_files(
    #[from(linux_target)] linux_target_res: Result<TargetTriple, ArtefactError>,
) {
    let linux_target = linux_target_res.expect("linux target triple should validate");

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
fn discover_library_files_rejects_missing(
    #[from(linux_target)] linux_target_res: Result<TargetTriple, ArtefactError>,
) {
    let linux_target = linux_target_res.expect("linux target triple should validate");

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

// Round-trip every generated epoch date through the inverse Gregorian formula.
proptest! {
    #[test]
    fn civil_from_epoch_round_trips_any_epoch_seconds(epoch_secs in any::<u64>()) {
        let (year, month, day) = civil_from_epoch(epoch_secs);
        let seconds_of_day = epoch_secs % 86_400;
        let hour = seconds_of_day / 3_600;
        let minute = seconds_of_day % 3_600 / 60;
        let second = seconds_of_day % 60;

        prop_assert!((1..=12).contains(&month));
        prop_assert!((1..=days_in_month(year, month)).contains(&day));
        prop_assert!(hour < 24);
        prop_assert!(minute < 60);
        prop_assert!(second < 60);
        prop_assert_eq!(days_from_civil(year, month, day), epoch_secs / 86_400);
    }
}

/// Convert a Gregorian date to days since the Unix epoch using era arithmetic.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = adjusted_year / 400;
    let year_of_era = adjusted_year - era * 400;
    let month_of_year = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_of_year + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Return the number of days in a Gregorian month.
fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_gregorian_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Return whether a year is a leap year in the Gregorian calendar.
fn is_gregorian_leap_year(year: u64) -> bool { year % 400 == 0 || year % 4 == 0 && year % 100 != 0 }

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

/// Preserve a broken-pipe error when reporting a successful package.
#[test]
fn run_returns_broken_pipe_from_success_report() {
    let dir = tempfile::tempdir().expect("temp dir");
    let library = dir.path().join("libwhitaker_suite.so");
    fs::write(&library, b"fake library").expect("write library");
    let cli = PackageCli {
        git_sha: "abc1234".to_owned(),
        toolchain: "nightly-2026-05-28".to_owned(),
        target: "x86_64-unknown-linux-gnu".to_owned(),
        output_dir: dir.path().join("dist"),
        generated_at: Some("2026-02-12T10:00:00Z".to_owned()),
        release_dir: None,
        library_files: vec![library],
    };
    let mut stdout = BrokenPipeWriter;

    let error = run_with_stdout(cli, &mut stdout).expect_err("broken stdout must be reported");

    assert!(
        matches!(error, PackageCliError::Stdout(source) if source.kind() == io::ErrorKind::BrokenPipe)
    );
}

/// Return a broken-pipe error for every write request.
struct BrokenPipeWriter;

impl io::Write for BrokenPipeWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
