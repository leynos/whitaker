//! Behaviour tests for installer metrics recording.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use whitaker_installer::install_metrics::{
    InstallMetrics, InstallMode, RecordOutcome, record_install_at_path,
};

#[derive(Default)]
struct InstallMetricsWorld {
    _temp_dir: Option<TempDir>,
    metrics_path: Option<PathBuf>,
    outcome: Option<RecordOutcome>,
    last_error: Option<String>,
    in_memory_metrics: Option<InstallMetrics>,
}

#[fixture]
fn world() -> InstallMetricsWorld {
    InstallMetricsWorld::default()
}

fn record_mode(world: &mut InstallMetricsWorld, mode: InstallMode, millis: u64) {
    let path = world.metrics_path.as_deref().expect("metrics path set");
    let result = record_install_at_path(path, mode, Duration::from_millis(millis));
    match result {
        Ok(outcome) => {
            world.in_memory_metrics = Some(outcome.metrics().clone());
            world.last_error = None;
            world.outcome = Some(outcome);
        }
        Err(error) => {
            world.last_error = Some(error.to_string());
            world.outcome = None;
        }
    }
}

#[given("an empty install metrics store")]
fn given_empty_store(world: &mut InstallMetricsWorld) {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    world.metrics_path = Some(temp_dir.path().join("metrics").join("install_metrics.json"));
    world._temp_dir = Some(temp_dir);
    world.outcome = None;
    world.last_error = None;
    world.in_memory_metrics = None;
}

#[given("a corrupt install metrics store")]
fn given_corrupt_store(world: &mut InstallMetricsWorld) {
    given_empty_store(world);
    let path = world.metrics_path.as_deref().expect("metrics path set");
    std::fs::create_dir_all(path.parent().expect("metrics parent exists")).expect("create parent");
    std::fs::write(path, "{not valid json").expect("write corrupt file");
}

#[given("a blocked install metrics path")]
fn given_blocked_path(world: &mut InstallMetricsWorld) {
    given_empty_store(world);
    let path = world.metrics_path.as_deref().expect("metrics path set");
    std::fs::create_dir_all(path).expect("create blocking directory");
}

#[given("a download install of {millis:u64} milliseconds is recorded")]
fn given_download_recorded(world: &mut InstallMetricsWorld, millis: u64) {
    record_mode(world, InstallMode::Download, millis);
}

#[given("an in-memory zero metrics aggregate")]
fn given_zero_metrics(world: &mut InstallMetricsWorld) {
    world.in_memory_metrics = Some(InstallMetrics::default());
}

#[when("a download install of {millis:u64} milliseconds is recorded")]
fn when_download_recorded(world: &mut InstallMetricsWorld, millis: u64) {
    record_mode(world, InstallMode::Download, millis);
}

#[when("a build install of {millis:u64} milliseconds is recorded")]
fn when_build_recorded(world: &mut InstallMetricsWorld, millis: u64) {
    record_mode(world, InstallMode::Build, millis);
}

#[when("download and build rates are calculated")]
fn when_rates_calculated(world: &mut InstallMetricsWorld) {
    let _ = world;
}

#[then("total installs is {expected:u64}")]
fn then_total_installs(world: &mut InstallMetricsWorld, expected: u64) {
    let metrics = world.in_memory_metrics.as_ref().expect("metrics available");
    assert_eq!(metrics.total_installs(), expected);
}

#[then("download installs is {expected:u64}")]
fn then_download_installs(world: &mut InstallMetricsWorld, expected: u64) {
    let metrics = world.in_memory_metrics.as_ref().expect("metrics available");
    assert_eq!(metrics.download_installs(), expected);
}

#[then("build installs is {expected:u64}")]
fn then_build_installs(world: &mut InstallMetricsWorld, expected: u64) {
    let metrics = world.in_memory_metrics.as_ref().expect("metrics available");
    assert_eq!(metrics.build_installs(), expected);
}

#[then("download rate is {expected:f64}")]
fn then_download_rate(world: &mut InstallMetricsWorld, expected: f64) {
    let metrics = world.in_memory_metrics.as_ref().expect("metrics available");
    assert!(
        (metrics.download_rate() - expected).abs() < f64::EPSILON,
        "expected {}, got {}",
        expected,
        metrics.download_rate()
    );
}

#[then("build rate is {expected:f64}")]
fn then_build_rate(world: &mut InstallMetricsWorld, expected: f64) {
    let metrics = world.in_memory_metrics.as_ref().expect("metrics available");
    assert!(
        (metrics.build_rate() - expected).abs() < f64::EPSILON,
        "expected {}, got {}",
        expected,
        metrics.build_rate()
    );
}

#[then("total installation time is {expected:u64} milliseconds")]
fn then_total_installation_time(world: &mut InstallMetricsWorld, expected: u64) {
    let metrics = world.in_memory_metrics.as_ref().expect("metrics available");
    assert_eq!(
        metrics.total_install_duration(),
        Duration::from_millis(expected)
    );
}

#[then("metrics recovery from corrupt file is true")]
fn then_recovered(world: &mut InstallMetricsWorld) {
    let outcome = world.outcome.as_ref().expect("recording outcome available");
    assert!(outcome.recovered_from_corrupt_file());
}

#[then("metrics recording fails")]
fn then_recording_fails(world: &mut InstallMetricsWorld) {
    assert!(
        world.last_error.is_some(),
        "expected recording to fail, got success outcome"
    );
}

#[scenario(
    path = "tests/features/install_metrics.feature",
    name = "Record a successful download install"
)]
fn scenario_download_install(world: InstallMetricsWorld) {
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
    name = "Report write failures as errors"
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
