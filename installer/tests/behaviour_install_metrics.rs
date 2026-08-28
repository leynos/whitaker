//! Behaviour tests for installer metrics recording.

use std::{path::PathBuf, time::Duration};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use whitaker_installer::install_metrics::{
    InstallMetrics, InstallMode, RecordOutcome, record_install_at_path,
};

/// Tolerance for comparing floating-point install rates.
const FLOAT_RATE_TOLERANCE: f64 = 1e-6;

#[derive(Default)]
struct InstallMetricsWorld {
    /// Owns the scenario's temporary metrics directory so it outlives the run.
    temp_dir: Option<TempDir>,
    metrics_path: Option<PathBuf>,
    outcome: Option<RecordOutcome>,
    last_error: Option<String>,
    in_memory_metrics: Option<InstallMetrics>,
    summary_line: Option<String>,
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> InstallMetricsWorld {
    InstallMetricsWorld::default()
}

/// Compare two values for equality, reporting a mismatch as an error.
fn ensure_eq<T, U>(actual: &T, expected: &U, context: &str) -> Result<(), String>
where
    T: PartialEq<U> + std::fmt::Debug + ?Sized,
    U: std::fmt::Debug + ?Sized,
{
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{context}: expected {expected:?}, got {actual:?}"))
    }
}

/// Borrow the configured metrics path, failing when no Given step has run.
fn metrics_path(world: &InstallMetricsWorld) -> Result<&std::path::Path, String> {
    world
        .metrics_path
        .as_deref()
        .ok_or_else(|| String::from("metrics path set"))
}

/// Borrow the aggregated metrics, failing when nothing has been recorded.
fn metrics(world: &InstallMetricsWorld) -> Result<&InstallMetrics, String> {
    world
        .in_memory_metrics
        .as_ref()
        .ok_or_else(|| String::from("metrics available"))
}

fn record_mode(
    world: &mut InstallMetricsWorld,
    mode: InstallMode,
    millis: u64,
) -> Result<(), String> {
    let path = metrics_path(world)?;
    let result = record_install_at_path(path, mode, Duration::from_millis(millis));
    match result {
        Ok(outcome) => {
            let metrics = outcome.metrics().clone();
            world.summary_line = Some(metrics.summary_line());
            world.in_memory_metrics = Some(metrics);
            world.last_error = None;
            world.outcome = Some(outcome);
        }
        Err(error) => {
            world.last_error = Some(error.to_string());
            world.outcome = None;
            world.summary_line = None;
            world.in_memory_metrics = None;
        }
    }
    Ok(())
}

#[given("an empty install metrics store")]
fn given_empty_store(world: &mut InstallMetricsWorld) -> Result<(), String> {
    let temp_dir = tempfile::tempdir().map_err(|error| format!("create temp dir: {error}"))?;
    world.metrics_path = Some(temp_dir.path().join("metrics").join("install_metrics.json"));
    world.temp_dir = Some(temp_dir);
    world.outcome = None;
    world.last_error = None;
    world.in_memory_metrics = None;
    world.summary_line = None;
    Ok(())
}

#[given("a corrupt install metrics store")]
fn given_corrupt_store(world: &mut InstallMetricsWorld) -> Result<(), String> {
    given_empty_store(world)?;
    let path = metrics_path(world)?;
    let parent = path
        .parent()
        .ok_or_else(|| String::from("metrics parent exists"))?;
    std::fs::create_dir_all(parent).map_err(|error| format!("create parent: {error}"))?;
    std::fs::write(path, "{not valid json").map_err(|error| format!("write corrupt file: {error}"))
}

#[given("a blocked install metrics path")]
fn given_blocked_path(world: &mut InstallMetricsWorld) -> Result<(), String> {
    given_empty_store(world)?;
    let path = metrics_path(world)?;
    std::fs::create_dir_all(path).map_err(|error| format!("create blocking directory: {error}"))
}

#[given("a download install of {millis:u64} milliseconds is recorded")]
fn given_download_recorded(world: &mut InstallMetricsWorld, millis: u64) -> Result<(), String> {
    record_mode(world, InstallMode::Download, millis)
}

#[given("an in-memory zero metrics aggregate")]
fn given_zero_metrics(world: &mut InstallMetricsWorld) {
    world.in_memory_metrics = Some(InstallMetrics::default());
}

#[when("a download install of {millis:u64} milliseconds is recorded")]
fn when_download_recorded(world: &mut InstallMetricsWorld, millis: u64) -> Result<(), String> {
    record_mode(world, InstallMode::Download, millis)
}

#[when("a build install of {millis:u64} milliseconds is recorded")]
fn when_build_recorded(world: &mut InstallMetricsWorld, millis: u64) -> Result<(), String> {
    record_mode(world, InstallMode::Build, millis)
}

#[when("download and build rates are calculated")]
fn when_rates_calculated(world: &mut InstallMetricsWorld) {
    let _ = world;
}

#[then("total installs is {expected:u64}")]
fn then_total_installs(world: &mut InstallMetricsWorld, expected: u64) -> Result<(), String> {
    ensure_eq(
        &metrics(world)?.total_installs(),
        &expected,
        "total installs",
    )
}

#[then("download installs is {expected:u64}")]
fn then_download_installs(world: &mut InstallMetricsWorld, expected: u64) -> Result<(), String> {
    ensure_eq(
        &metrics(world)?.download_installs(),
        &expected,
        "download installs",
    )
}

#[then("build installs is {expected:u64}")]
fn then_build_installs(world: &mut InstallMetricsWorld, expected: u64) -> Result<(), String> {
    ensure_eq(
        &metrics(world)?.build_installs(),
        &expected,
        "build installs",
    )
}

/// Compares a floating-point rate against its expected value with tolerance.
fn ensure_rate(actual: f64, expected: f64, context: &str) -> Result<(), String> {
    if (actual - expected).abs() < FLOAT_RATE_TOLERANCE {
        Ok(())
    } else {
        Err(format!("{context}: expected {expected}, got {actual}"))
    }
}

#[then("download rate is {expected:f64}")]
fn then_download_rate(world: &mut InstallMetricsWorld, expected: f64) -> Result<(), String> {
    ensure_rate(metrics(world)?.download_rate(), expected, "download rate")
}

#[then("build rate is {expected:f64}")]
fn then_build_rate(world: &mut InstallMetricsWorld, expected: f64) -> Result<(), String> {
    ensure_rate(metrics(world)?.build_rate(), expected, "build rate")
}

#[then("total installation time is {expected:u64} milliseconds")]
fn then_total_installation_time(
    world: &mut InstallMetricsWorld,
    expected: u64,
) -> Result<(), String> {
    ensure_eq(
        &metrics(world)?.total_install_duration(),
        &Duration::from_millis(expected),
        "total installation time",
    )
}

#[then("metrics recovery from corrupt file is true")]
fn then_recovered(world: &mut InstallMetricsWorld) -> Result<(), String> {
    let outcome = world
        .outcome
        .as_ref()
        .ok_or_else(|| String::from("recording outcome available"))?;
    if outcome.recovered_from_corrupt_file() {
        Ok(())
    } else {
        Err(String::from(
            "expected recovery from a corrupt metrics file",
        ))
    }
}

#[then("metrics recording fails")]
fn then_recording_fails(world: &mut InstallMetricsWorld) -> Result<(), String> {
    if world.last_error.is_some() {
        Ok(())
    } else {
        Err(String::from(
            "expected recording to fail, got success outcome",
        ))
    }
}

#[then("summary line contains \"{expected}\"")]
fn then_summary_line_contains(
    world: &mut InstallMetricsWorld,
    expected: String,
) -> Result<(), String> {
    let summary = world
        .summary_line
        .as_deref()
        .ok_or_else(|| String::from("summary line is available"))?;
    if summary.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "expected summary line to contain {expected:?}, got {summary:?}"
        ))
    }
}

#[then("warning text contains \"{expected}\"")]
fn then_warning_text_contains(
    world: &mut InstallMetricsWorld,
    expected: String,
) -> Result<(), String> {
    let error = world
        .last_error
        .as_deref()
        .ok_or_else(|| String::from("metrics recording error should be available"))?;
    let warning_text = format!("Warning: could not record install metrics: {error}");
    if warning_text.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "expected warning text to contain {expected:?}, got {warning_text:?}"
        ))
    }
}

#[scenario(
    path = "tests/features/install_metrics.feature",
    name = "Record a successful prebuilt-download install"
)]
fn scenario_download_install(world: InstallMetricsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/install_metrics.feature",
    name = "Record a successful build-only install"
)]
fn scenario_build_only_install(world: InstallMetricsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/install_metrics.feature",
    name = "Record download and build installs"
)]
fn scenario_download_and_build_installs(world: InstallMetricsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/install_metrics.feature",
    name = "Recover from a corrupt metrics file"
)]
fn scenario_recover_from_corrupt_file(world: InstallMetricsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/install_metrics.feature",
    name = "Report write failures as warning text"
)]
fn scenario_report_write_failures(world: InstallMetricsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/install_metrics.feature",
    name = "Zero-state rates are zero"
)]
fn scenario_zero_state_rates(world: InstallMetricsWorld) {
    let _ = world;
}
