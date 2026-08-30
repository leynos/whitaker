//! Installer metrics for download-versus-build outcomes and install duration.
//!
//! This module records local, aggregate metrics for successful installer runs.
//! Metrics are stored in Whitaker's data directory at:
//! `<data_dir>/metrics/install_metrics.json`.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::dirs::BaseDirs;

const METRICS_DIRNAME: &str = "metrics";
const METRICS_FILENAME: &str = "install_metrics.json";

#[path = "install_metrics_error.rs"]
mod error;
pub use error::InstallMetricsError;
/// Terminal installation path used for metrics accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    /// The install succeeded via prebuilt artefact download.
    Download,
    /// The install succeeded via local build and staging.
    Build,
}

/// Aggregate installer metrics stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InstallMetrics {
    total_installs: u64,
    download_installs: u64,
    build_installs: u64,
    total_install_millis: u64,
}
impl InstallMetrics {
    /// Returns the number of successful installs.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Download, Duration::from_millis(250));
    /// assert_eq!(metrics.total_installs(), 1);
    /// ```
    #[must_use]
    pub const fn total_installs(&self) -> u64 { self.total_installs }

    /// Returns the number of successful prebuilt-download installs.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Download, Duration::from_millis(250));
    /// assert_eq!(metrics.download_installs(), 1);
    /// ```
    #[must_use]
    pub const fn download_installs(&self) -> u64 { self.download_installs }

    /// Returns the number of successful local-build installs.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Build, Duration::from_millis(250));
    /// assert_eq!(metrics.build_installs(), 1);
    /// ```
    #[must_use]
    pub const fn build_installs(&self) -> u64 { self.build_installs }

    /// Returns total cumulative install duration.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use whitaker_installer::install_metrics::InstallMetrics;
    ///
    /// assert_eq!(
    ///     InstallMetrics::default().total_install_duration(),
    ///     Duration::from_secs(0)
    /// );
    /// ```
    #[must_use]
    pub const fn total_install_duration(&self) -> Duration {
        Duration::from_millis(self.total_install_millis)
    }

    /// Returns `download_installs / total_installs` in permille (0–1000).
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::install_metrics::InstallMetrics;
    ///
    /// assert_eq!(InstallMetrics::default().download_rate_permille(), 0);
    /// ```
    #[must_use]
    pub const fn download_rate_permille(&self) -> u64 {
        rate_permille(self.download_installs, self.total_installs)
    }

    /// Returns `build_installs / total_installs` in permille (0–1000).
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::install_metrics::InstallMetrics;
    ///
    /// assert_eq!(InstallMetrics::default().build_rate_permille(), 0);
    /// ```
    #[must_use]
    pub const fn build_rate_permille(&self) -> u64 {
        rate_permille(self.build_installs, self.total_installs)
    }

    /// Records one successful install event.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Download, Duration::from_millis(500));
    /// metrics.record_install(InstallMode::Build, Duration::from_millis(1000));
    /// assert_eq!(metrics.total_installs(), 2);
    /// assert_eq!(metrics.download_installs(), 1);
    /// assert_eq!(metrics.build_installs(), 1);
    /// ```
    pub fn record_install(&mut self, mode: InstallMode, duration: Duration) {
        self.total_installs = self.total_installs.saturating_add(1);
        match mode {
            InstallMode::Download => {
                self.download_installs = self.download_installs.saturating_add(1);
            }
            InstallMode::Build => {
                self.build_installs = self.build_installs.saturating_add(1);
            }
        }
        self.total_install_millis = self
            .total_install_millis
            .saturating_add(duration_to_millis(duration));
    }

    /// Returns a human-readable installer metrics summary line.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Download, Duration::from_millis(500));
    /// let summary = metrics.summary_line();
    /// assert!(summary.contains("download 1/1 (100.0%)"));
    /// assert!(summary.contains("total installation time 0.500s"));
    /// ```
    #[must_use]
    pub fn summary_line(&self) -> String {
        let download_percent = format_permille_as_percent(self.download_rate_permille());
        let build_percent = format_permille_as_percent(self.build_rate_permille());
        format!(
            concat!(
                "Install metrics: download {}/{} ({}%), build {}/{} ({}%), ",
                "total installation time {}"
            ),
            self.download_installs,
            self.total_installs,
            download_percent,
            self.build_installs,
            self.total_installs,
            build_percent,
            format_duration(self.total_install_duration()),
        )
    }
}

/// Outcome details returned after recording metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOutcome {
    metrics: InstallMetrics,
    recovered_from_corrupt_file: bool,
}
impl RecordOutcome {
    /// Returns the updated aggregate metrics.
    #[must_use]
    pub const fn metrics(&self) -> &InstallMetrics { &self.metrics }

    /// Returns true when a malformed metrics file was reset to defaults.
    #[must_use]
    pub const fn recovered_from_corrupt_file(&self) -> bool { self.recovered_from_corrupt_file }
}

/// Records one successful install in Whitaker's metrics store.
///
/// # Errors
///
/// Returns an [`InstallMetricsError`] when the data directory is missing or
/// the metrics file cannot be created, locked, read, or written.
pub fn record_install(
    dirs: &dyn BaseDirs,
    mode: InstallMode,
    duration: Duration,
) -> Result<RecordOutcome, InstallMetricsError> {
    let metrics_path = metrics_path(dirs)?;
    record_install_at_path(&metrics_path, mode, duration)
}

/// Records one successful install at an explicit metrics file path.
///
/// # Errors
///
/// Returns an [`InstallMetricsError`] when the metrics directory or file
/// cannot be created, or the file cannot be locked, read, or written.
pub fn record_install_at_path(
    metrics_path: &Path,
    mode: InstallMode,
    duration: Duration,
) -> Result<RecordOutcome, InstallMetricsError> {
    ensure_metrics_directory(metrics_path)?;
    let mut metrics_file = open_metrics_file(metrics_path)?;
    // Use standard-library advisory locking to serialize the read-modify-write
    // cycle across concurrent installer processes.
    metrics_file
        .lock_exclusive()
        .map_err(|source| InstallMetricsError::LockMetrics {
            path: metrics_path.to_path_buf(),
            source,
        })?;

    let (mut metrics, recovered_from_corrupt_file) = load_metrics(metrics_path, &mut metrics_file)?;
    metrics.record_install(mode, duration);
    persist_metrics(metrics_path, &mut metrics_file, &metrics)?;

    Ok(RecordOutcome {
        metrics,
        recovered_from_corrupt_file,
    })
}

fn metrics_path(dirs: &dyn BaseDirs) -> Result<PathBuf, InstallMetricsError> {
    let data_dir = dirs
        .whitaker_data()
        .ok_or(InstallMetricsError::MissingDataDirectory)?;
    Ok(data_dir.join(METRICS_DIRNAME).join(METRICS_FILENAME))
}

fn ensure_metrics_directory(metrics_path: &Path) -> Result<(), InstallMetricsError> {
    let parent = metrics_path
        .parent()
        .ok_or_else(|| InstallMetricsError::CreateDirectory {
            path: PathBuf::new(),
            source: std::io::Error::other("metrics file path has no parent"),
        })?;

    std::fs::create_dir_all(parent).map_err(|source| InstallMetricsError::CreateDirectory {
        path: parent.to_path_buf(),
        source,
    })
}

fn open_metrics_file(metrics_path: &Path) -> Result<File, InstallMetricsError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(metrics_path)
        .map_err(|source| InstallMetricsError::ReadMetrics {
            path: metrics_path.to_path_buf(),
            source,
        })
}

fn load_metrics(
    metrics_path: &Path,
    metrics_file: &mut File,
) -> Result<(InstallMetrics, bool), InstallMetricsError> {
    metrics_file
        .seek(SeekFrom::Start(0))
        .map_err(|source| InstallMetricsError::ReadMetrics {
            path: metrics_path.to_path_buf(),
            source,
        })?;

    let mut content = String::new();
    metrics_file
        .read_to_string(&mut content)
        .map_err(|source| InstallMetricsError::ReadMetrics {
            path: metrics_path.to_path_buf(),
            source,
        })?;

    if content.trim().is_empty() {
        return Ok((InstallMetrics::default(), false));
    }

    serde_json::from_str::<InstallMetrics>(&content).map_or_else(
        |_| Ok((InstallMetrics::default(), true)),
        |metrics| Ok((metrics, false)),
    )
}

fn persist_metrics(
    metrics_path: &Path,
    metrics_file: &mut File,
    metrics: &InstallMetrics,
) -> Result<(), InstallMetricsError> {
    let json = serde_json::to_string_pretty(metrics)
        .map_err(|source| InstallMetricsError::SerializeMetrics { source })?;
    metrics_file
        .set_len(0)
        .and_then(|()| metrics_file.seek(SeekFrom::Start(0)).map(|_| ()))
        .and_then(|()| metrics_file.write_all(json.as_bytes()))
        .and_then(|()| metrics_file.sync_data())
        .map_err(|source| InstallMetricsError::WriteMetrics {
            path: metrics_path.to_path_buf(),
            source,
        })
}

/// Returns `part / whole` in permille (parts per thousand), or zero when
/// `whole` is zero. The product saturates rather than overflowing.
const fn rate_permille(part: u64, whole: u64) -> u64 {
    if whole == 0 {
        0
    } else {
        part.saturating_mul(1000).div_euclid(whole)
    }
}

/// Renders a permille value as a percentage with one decimal place.
fn format_permille_as_percent(permille: u64) -> String {
    format!("{}.{}", permille.div_euclid(10), permille.rem_euclid(10))
}

fn duration_to_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let millis = duration.subsec_millis();
    let hours = total_seconds.div_euclid(3600);
    let minutes = total_seconds.rem_euclid(3600).div_euclid(60);
    let seconds = total_seconds.rem_euclid(60);

    if should_format_with_hours(hours) {
        return format!("{hours}h {minutes}m {seconds}.{millis:03}s");
    }
    if should_format_with_minutes(minutes) {
        return format!("{minutes}m {seconds}.{millis:03}s");
    }
    format!("{seconds}.{millis:03}s")
}

const fn should_format_with_hours(hours: u64) -> bool { hours > 0 }

const fn should_format_with_minutes(minutes: u64) -> bool { minutes > 0 }
