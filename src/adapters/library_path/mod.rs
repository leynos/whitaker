//! Resolution of the Dylint library path used when forwarding a command.
//!
//! A forwarded invocation needs `DYLINT_LIBRARY_PATH` to name every directory
//! that may hold lint libraries, because Dylint selects among them by the
//! toolchain a project declares. The directories come from three sources in
//! precedence order: an explicit `DYLINT_LIBRARY_PATH`, the directories an
//! earlier installation recorded, and the directories this build or a prebuilt
//! release would have produced.
//!
//! The result is an explicit [`LibraryPath`] rather than an `Option`, so the
//! caller can tell an authoritative user setting from a resolved list and from
//! the normal "nothing installed yet" case. Only the last of those leaves
//! Dylint to find its own libraries.

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use camino::Utf8PathBuf;
use whitaker_installer::{
    dependency_binaries::host_target,
    dirs::BaseDirs,
    installation_record::installed_paths,
    list::try_detect_active_toolchain,
    prebuilt_path::prebuilt_library_dir,
    stager::default_target_dir,
};

use crate::ports::CliError;

/// The library path a forwarded invocation resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryPath {
    /// The user set `DYLINT_LIBRARY_PATH`; it stays authoritative.
    Explicit(OsString),
    /// One or more existing directories, joined for Dylint to select from.
    Resolved {
        /// The joined platform path list exported to Dylint.
        path: OsString,
        /// How many directories survived the existence and representability
        /// filters.
        candidates: usize,
    },
    /// No candidate directory exists, so Dylint must find its own libraries.
    NoCandidate,
}

/// Which resolution outcome produced a [`LibraryPath`].
///
/// The kind is a bounded label for diagnostics and metrics; it deliberately
/// carries no paths, so it is safe to record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPathKind {
    /// An explicit `DYLINT_LIBRARY_PATH` setting.
    Explicit,
    /// Directories resolved from the installation record and defaults.
    Resolved,
    /// No candidate directory exists.
    NoCandidate,
}

impl LibraryPathKind {
    /// The bounded label recorded for this outcome.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::library_path::LibraryPathKind;
    ///
    /// assert_eq!(LibraryPathKind::Resolved.label(), "resolved");
    /// assert_eq!(LibraryPathKind::NoCandidate.label(), "no_candidate");
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Resolved => "resolved",
            Self::NoCandidate => "no_candidate",
        }
    }
}

impl LibraryPath {
    /// Borrow the value to export as `DYLINT_LIBRARY_PATH`, if there is one.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// use whitaker::adapters::library_path::LibraryPath;
    ///
    /// let path = LibraryPath::Resolved {
    ///     path: OsString::from("/lints"),
    ///     candidates: 1,
    /// };
    /// assert_eq!(path.as_deref(), Some(std::ffi::OsStr::new("/lints")));
    /// assert_eq!(LibraryPath::NoCandidate.as_deref(), None);
    /// ```
    #[must_use]
    pub fn as_deref(&self) -> Option<&OsStr> {
        match self {
            Self::Explicit(path) | Self::Resolved { path, .. } => Some(path.as_os_str()),
            Self::NoCandidate => None,
        }
    }

    /// Count the directories this resolution contributed.
    ///
    /// An explicit setting counts as one source however many entries it
    /// names, because the value is passed through untouched rather than
    /// assembled here.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::library_path::LibraryPath;
    ///
    /// assert_eq!(LibraryPath::NoCandidate.candidate_count(), 0);
    /// ```
    #[must_use]
    pub const fn candidate_count(&self) -> usize {
        match self {
            Self::Explicit(_) => 1,
            Self::Resolved { candidates, .. } => *candidates,
            Self::NoCandidate => 0,
        }
    }

    /// Report which outcome produced this value.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// use whitaker::adapters::library_path::{LibraryPath, LibraryPathKind};
    ///
    /// assert_eq!(
    ///     LibraryPath::Explicit(OsString::from("/lints")).kind(),
    ///     LibraryPathKind::Explicit
    /// );
    /// ```
    #[must_use]
    pub const fn kind(&self) -> LibraryPathKind {
        match self {
            Self::Explicit(_) => LibraryPathKind::Explicit,
            Self::Resolved { .. } => LibraryPathKind::Resolved,
            Self::NoCandidate => LibraryPathKind::NoCandidate,
        }
    }
}

/// Resolve the library path for a forwarded invocation.
///
/// # Errors
///
/// Returns [`CliError::InstallationRecord`] when an installation record exists
/// but cannot be locked, read, or parsed, [`CliError::ToolchainDetection`] when
/// a `rust-toolchain.toml` exists but cannot be read or parsed,
/// [`CliError::PrebuiltLibraryPath`] when the prebuilt directory cannot be
/// derived, and [`CliError::LibraryPath`] when the surviving candidates cannot
/// be joined. A missing record, and a directory that declares no toolchain, are
/// not errors: they simply mean nothing has been installed for this project
/// yet.
pub(super) fn active_library_path(dirs: Option<&dyn BaseDirs>) -> Result<LibraryPath, CliError> {
    if let Some(explicit) = explicit_library_path() {
        return Ok(LibraryPath::Explicit(explicit));
    }
    let recorded = recorded_library_paths(dirs)?;
    let toolchain =
        try_detect_active_toolchain().map_err(|source| CliError::ToolchainDetection { source })?;
    let build = toolchain.as_deref().and_then(build_library_path);
    let prebuilt = toolchain
        .as_deref()
        .map(|active| prebuilt_library_path(dirs, active))
        .transpose()?
        .flatten();
    resolve_library_path(recorded, build, prebuilt)
}

/// Read an explicit `DYLINT_LIBRARY_PATH`, ignoring an empty setting.
///
/// An empty value is what a shell leaves behind for `VAR=`, and treating it as
/// authoritative would suppress every resolved directory.
fn explicit_library_path() -> Option<OsString> {
    std::env::var_os("DYLINT_LIBRARY_PATH").filter(|path| !path.is_empty())
}

/// Reads the per-toolchain staging directories recorded by earlier installs.
///
/// An unavailable platform data directory yields no recorded paths, matching
/// the "nothing installed" case. A record that exists but fails to load is
/// propagated so the caller can report it instead of silently forwarding with
/// an empty library path.
///
/// The directory provider is a parameter so callers pass one resolved view,
/// and so the record's error paths can be exercised with a directory a test
/// owns. An environment-driven redirect is not portable: Windows resolves the
/// data directory from the known-folder API and ignores `XDG_DATA_HOME`.
fn recorded_library_paths(dirs: Option<&dyn BaseDirs>) -> Result<Vec<Utf8PathBuf>, CliError> {
    // No provider means no record to read, so there is no read outcome to
    // report either.
    let Some(provider) = dirs else {
        return Ok(Vec::new());
    };
    let recorded = installed_paths(provider);
    super::telemetry::record_installation_record_read(recorded.is_ok());
    recorded.map_err(|source| CliError::InstallationRecord { source })
}

/// The default target directory's release output for `toolchain`.
///
/// A host with no resolvable target directory simply has no build candidate;
/// that is a normal state rather than a failure.
fn build_library_path(toolchain: &str) -> Option<Utf8PathBuf> {
    default_target_dir().map(|target_dir| target_dir.join(toolchain).join("release"))
}

/// Resolves the prebuilt library directory for the active host and toolchain.
///
/// # Errors
///
/// Returns [`CliError::PrebuiltLibraryPath`] when the data directory is
/// unavailable or not UTF-8. A host whose target triple is unknown has no
/// prebuilt candidate, which is not an error.
fn prebuilt_library_path(
    dirs: Option<&dyn BaseDirs>,
    toolchain: &str,
) -> Result<Option<Utf8PathBuf>, CliError> {
    let Some(target) = host_target() else {
        return Ok(None);
    };
    let Some(provider) = dirs else {
        return Ok(None);
    };
    prebuilt_library_dir(provider, toolchain, target.as_str())
        .map(Some)
        .map_err(|error| CliError::PrebuiltLibraryPath {
            message: error.to_string(),
        })
}

/// Combine recorded and default directories so Dylint can select by toolchain.
///
/// # Errors
///
/// Returns [`CliError::LibraryPath`] when the surviving candidates cannot be
/// joined into one platform path list.
fn resolve_library_path(
    recorded_paths: Vec<Utf8PathBuf>,
    build_path: Option<Utf8PathBuf>,
    prebuilt_path: Option<Utf8PathBuf>,
) -> Result<LibraryPath, CliError> {
    let mut candidates = recorded_paths;
    for candidate in [build_path, prebuilt_path].into_iter().flatten() {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    join_existing_directories(candidates)
}

/// Join every existing candidate directory so Dylint can select by toolchain.
///
/// A candidate that is missing, is not a directory, or cannot be represented
/// in a platform path list is dropped rather than failing the whole
/// resolution: one stale or unrepresentable recorded path must not stop
/// forwarding for the directories that are still valid. Dropping every
/// candidate is the [`LibraryPath::NoCandidate`] case.
///
/// # Errors
///
/// Returns [`CliError::LibraryPath`] when the surviving candidates cannot be
/// joined. Each survivor is individually representable, so this is a
/// defensive report rather than an expected outcome.
fn join_existing_directories(candidates: Vec<Utf8PathBuf>) -> Result<LibraryPath, CliError> {
    let existing: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|path| path.is_absolute() && path.is_dir())
        .map(Utf8PathBuf::into_std_path_buf)
        .filter(|path| std::env::join_paths([path]).is_ok())
        .collect();
    if existing.is_empty() {
        return Ok(LibraryPath::NoCandidate);
    }
    let candidate_count = existing.len();
    std::env::join_paths(existing)
        .map(|path| LibraryPath::Resolved {
            path,
            candidates: candidate_count,
        })
        .map_err(|source| CliError::LibraryPath { source })
}

#[cfg(test)]
mod tests;
