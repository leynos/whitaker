//! Unit tests for installer metrics persistence and aggregation.

use std::{io::ErrorKind, path::PathBuf, time::Duration};

use rstest::{fixture, rstest};

use crate::install_metrics::{
    InstallMetrics, InstallMetricsError, InstallMode, record_install_at_path,
};

struct MetricsPathFixture {
    _temp_dir: tempfile::TempDir,
    metrics_path: PathBuf,
}

#[fixture]
fn metrics_path_fixture() -> std::io::Result<MetricsPathFixture> {
    let temp_dir = tempfile::tempdir()?;
    let metrics_path = temp_dir.path().join("metrics").join("install_metrics.json");
    Ok(MetricsPathFixture {
        _temp_dir: temp_dir,
        metrics_path,
    })
}

#[test]
fn zero_state_rates_are_zero() {
    let metrics = InstallMetrics::default();
    assert!(
        metrics.download_rate().abs() < f64::EPSILON,
        "zero-state download rate"
    );
    assert!(
        metrics.build_rate().abs() < f64::EPSILON,
        "zero-state build rate"
    );
}

#[test]
fn record_install_updates_counts_and_duration() {
    let mut metrics = InstallMetrics::default();
    metrics.record_install(InstallMode::Download, Duration::from_millis(1250));
    metrics.record_install(InstallMode::Build, Duration::from_millis(750));

    assert_eq!(metrics.total_installs(), 2);
    assert_eq!(metrics.download_installs(), 1);
    assert_eq!(metrics.build_installs(), 1);
    assert_eq!(metrics.total_install_duration(), Duration::from_secs(2));
    assert!(
        (metrics.download_rate() - 0.5).abs() < f64::EPSILON,
        "download rate"
    );
    assert!(
        (metrics.build_rate() - 0.5).abs() < f64::EPSILON,
        "build rate"
    );
}

#[rstest]
fn record_install_at_path_creates_metrics_file(
    #[from(metrics_path_fixture)] metrics_path_res: std::io::Result<MetricsPathFixture>,
) {
    let metrics_path_fixture = metrics_path_res.expect("metrics fixture should be created");

    let result = record_install_at_path(
        &metrics_path_fixture.metrics_path,
        InstallMode::Download,
        Duration::from_millis(250),
    )
    .expect("record install");

    assert_eq!(result.metrics().total_installs(), 1);
    assert!(metrics_path_fixture.metrics_path.exists());
}

#[rstest]
fn malformed_metrics_file_is_reset_and_recovered(
    #[from(metrics_path_fixture)] metrics_path_res: std::io::Result<MetricsPathFixture>,
) {
    let metrics_path_fixture = metrics_path_res.expect("metrics fixture should be created");

    std::fs::create_dir_all(
        metrics_path_fixture
            .metrics_path
            .parent()
            .expect("parent path"),
    )
    .expect("create parent");
    std::fs::write(&metrics_path_fixture.metrics_path, "{not valid json")
        .expect("write corrupt data");

    let result = record_install_at_path(
        &metrics_path_fixture.metrics_path,
        InstallMode::Build,
        Duration::from_millis(500),
    )
    .expect("record install from corrupt file");

    assert!(result.recovered_from_corrupt_file());
    assert_eq!(result.metrics().total_installs(), 1);
    assert_eq!(result.metrics().build_installs(), 1);
}

#[rstest]
fn persistence_failure_is_reported(
    #[from(metrics_path_fixture)] metrics_path_res: std::io::Result<MetricsPathFixture>,
) {
    let metrics_path_fixture = metrics_path_res.expect("metrics fixture should be created");

    std::fs::create_dir_all(&metrics_path_fixture.metrics_path).expect("create blocking directory");

    let error = record_install_at_path(
        &metrics_path_fixture.metrics_path,
        InstallMode::Build,
        Duration::from_secs(1),
    )
    .expect_err("expected persistence failure");

    match error {
        InstallMetricsError::ReadMetrics { source, .. }
        | InstallMetricsError::WriteMetrics { source, .. }
        | InstallMetricsError::LockMetrics { source, .. } => {
            assert!(
                matches!(
                    source.kind(),
                    ErrorKind::IsADirectory | ErrorKind::PermissionDenied
                ),
                "unexpected error kind: {}",
                source.kind()
            );
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[rstest]
fn summary_line_includes_rates_and_total_time(
    #[from(metrics_path_fixture)] metrics_path_res: std::io::Result<MetricsPathFixture>,
) {
    let metrics_path_fixture = metrics_path_res.expect("metrics fixture should be created");

    record_install_at_path(
        &metrics_path_fixture.metrics_path,
        InstallMode::Download,
        Duration::from_millis(1500),
    )
    .expect("record download install");
    let outcome = record_install_at_path(
        &metrics_path_fixture.metrics_path,
        InstallMode::Build,
        Duration::from_millis(500),
    )
    .expect("record build install");

    let summary = outcome.metrics().summary_line();
    assert!(summary.contains("download 1/2 (50.0%)"));
    assert!(summary.contains("build 1/2 (50.0%)"));
    assert!(summary.contains("total installation time 2.000s"));
}

#[rstest]
fn long_durations_saturate_total_install_time(
    #[from(metrics_path_fixture)] metrics_path_res: std::io::Result<MetricsPathFixture>,
) {
    let metrics_path_fixture = metrics_path_res.expect("metrics fixture should be created");

    std::fs::create_dir_all(
        metrics_path_fixture
            .metrics_path
            .parent()
            .expect("metrics parent"),
    )
    .expect("create metrics parent");
    std::fs::write(
        &metrics_path_fixture.metrics_path,
        format!(
            concat!(
                "{{",
                "\"total_installs\":1,",
                "\"download_installs\":0,",
                "\"build_installs\":1,",
                "\"total_install_millis\":{}",
                "}}"
            ),
            u64::MAX - 10
        ),
    )
    .expect("write near-saturated metrics");

    let outcome = record_install_at_path(
        &metrics_path_fixture.metrics_path,
        InstallMode::Build,
        Duration::from_millis(100),
    )
    .expect("record install on near-saturated metrics");
    assert_eq!(
        outcome.metrics().total_install_duration(),
        Duration::from_millis(u64::MAX)
    );
}

#[rstest]
fn concurrent_records_do_not_lose_updates(
    #[from(metrics_path_fixture)] metrics_path_res: std::io::Result<MetricsPathFixture>,
) {
    let metrics_path_fixture = metrics_path_res.expect("metrics fixture should be created");

    let path = metrics_path_fixture.metrics_path;
    let mut threads = Vec::new();

    for _ in 0..4 {
        let writer_path = path.clone();
        threads.push(std::thread::spawn(move || {
            for _ in 0..20 {
                record_install_at_path(
                    &writer_path,
                    InstallMode::Download,
                    Duration::from_millis(1),
                )
                .expect("record install from concurrent writer");
            }
        }));
    }

    for thread in threads {
        thread.join().expect("join concurrent thread");
    }

    let content = std::fs::read_to_string(path).expect("read metrics file");
    let metrics: InstallMetrics = serde_json::from_str(&content).expect("deserialize metrics");
    assert_eq!(metrics.total_installs(), 80);
    assert_eq!(metrics.download_installs(), 80);
    assert_eq!(metrics.build_installs(), 0);
}
