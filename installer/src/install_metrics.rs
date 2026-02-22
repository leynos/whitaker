//! Installer metrics for download-versus-build outcomes and install duration.
//!
//! This module records local, aggregate metrics for successful installer runs.
//! Metrics are stored in Whitaker's data directory at:
//! `<data_dir>/metrics/install_metrics.json`.

use crate::dirs::BaseDirs;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const METRICS_DIRNAME: &str = "metrics";
const METRICS_FILENAME: &str = "install_metrics.json";

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
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Download, Duration::from_millis(250));
    /// assert_eq!(metrics.total_installs(), 1);
    /// ```
    #[must_use]
    pub fn total_installs(&self) -> u64 {
        self.total_installs
    }

    /// Returns the number of successful prebuilt-download installs.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Download, Duration::from_millis(250));
    /// assert_eq!(metrics.download_installs(), 1);
    /// ```
    #[must_use]
    pub fn download_installs(&self) -> u64 {
        self.download_installs
    }

    /// Returns the number of successful local-build installs.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use whitaker_installer::install_metrics::{InstallMetrics, InstallMode};
    ///
    /// let mut metrics = InstallMetrics::default();
    /// metrics.record_install(InstallMode::Build, Duration::from_millis(250));
    /// assert_eq!(metrics.build_installs(), 1);
    /// ```
    #[must_use]
    pub fn build_installs(&self) -> u64 {
        self.build_installs
    }

    /// Returns total cumulative install duration.
    #[must_use]
    pub fn total_install_duration(&self) -> Duration {
        Duration::from_millis(self.total_install_millis)
    }

    /// Returns `download_installs / total_installs`.
    #[must_use]
    pub fn download_rate(&self) -> f64 {
        rate(self.download_installs, self.total_installs)
    }

    /// Returns `build_installs / total_installs`.
    #[must_use]
    pub fn build_rate(&self) -> f64 {
        rate(self.build_installs, self.total_installs)
    }

    /// Records one successful install event.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
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
        format!(
            concat!(
                "Install metrics: download {}/{} ({:.1}%), build {}/{} ({:.1}%), ",
                "total installation time {}"
            ),
            self.download_installs,
            self.total_installs,
            self.download_rate() * 100.0,
            self.build_installs,
            self.total_installs,
            self.build_rate() * 100.0,
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
    pub fn metrics(&self) -> &InstallMetrics {
        &self.metrics
    }

    /// Returns true when a malformed metrics file was reset to defaults.
    #[must_use]
    pub fn recovered_from_corrupt_file(&self) -> bool {
        self.recovered_from_corrupt_file
    }
}

/// Errors that prevent metrics persistence.
#[derive(Debug, thiserror::Error)]
pub enum InstallMetricsError {
    /// Whitaker data directory could not be resolved.
    #[error("could not determine Whitaker data directory")]
    MissingDataDirectory,

    /// Creating the metrics directory failed.
    #[error("failed to create metrics directory {path}: {source}")]
    CreateDirectory {
        /// Directory path that could not be created.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Reading the metrics file failed.
    #[error("failed to read metrics file {path}: {source}")]
    ReadMetrics {
        /// File path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Locking the metrics file failed.
    #[error("failed to lock metrics file {path}: {source}")]
    LockMetrics {
        /// File path that could not be locked.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Serializing metrics failed.
    #[error("failed to serialize metrics: {source}")]
    SerializeMetrics {
        /// Underlying serialization error.
        #[source]
        source: serde_json::Error,
    },

    /// Writing the metrics file failed.
    #[error("failed to write metrics file {path}: {source}")]
    WriteMetrics {
        /// File path that could not be written.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Records one successful install in Whitaker's metrics store.
pub fn record_install(
    dirs: &dyn BaseDirs,
    mode: InstallMode,
    duration: Duration,
) -> Result<RecordOutcome, InstallMetricsError> {
    let metrics_path = metrics_path(dirs)?;
    record_install_at_path(&metrics_path, mode, duration)
}

/// Records one successful install at an explicit metrics file path.
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
        .whitaker_data_dir()
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

    match serde_json::from_str::<InstallMetrics>(&content) {
        Ok(metrics) => Ok((metrics, false)),
        Err(_) => Ok((InstallMetrics::default(), true)),
    }
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

fn rate(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

fn duration_to_millis(duration: Duration) -> u64 {
    match u64::try_from(duration.as_millis()) {
        Ok(millis) => millis,
        Err(_) => u64::MAX,
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let millis = duration.subsec_millis();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if should_format_with_hours(hours) {
        return format!("{hours}h {minutes}m {seconds}.{millis:03}s");
    }
    if should_format_with_minutes(hours, minutes) {
        return format!("{minutes}m {seconds}.{millis:03}s");
    }
    format!("{seconds}.{millis:03}s")
}

fn should_format_with_hours(hours: u64) -> bool {
    hours > 0
}

fn should_format_with_minutes(hours: u64, minutes: u64) -> bool {
    hours == 0 && minutes > 0
}
