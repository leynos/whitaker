//! Reading a manifest from disk, with every failure carried as a typed error.
//!
//! The scan's two kinds of input fail for different reasons, and conflating
//! them is what this module exists to prevent.
//!
//! A **fixture** manifest is a string literal in this crate's source. The only
//! way it can fail to parse is that someone edited it wrongly, which is a
//! defect in the test rather than a condition under test, so [`parse_manifest`]
//! panics and names the defect. Its callers include `#[should_panic]` cases,
//! which the workspace's `panic_in_result_fn` forbids from returning `Result`,
//! so a fallible fixture parser would not even be usable there.
//!
//! A manifest **read from disk** is environmental. The file can be missing,
//! refused, or written by a different branch, and none of those is a defect in
//! the test. [`load_manifest`] reports them as [`DiscoveryError`], so the
//! `Result`-returning discovery queries that read a manifest propagate a real
//! error instead of panicking for a reason their signature does not mention.
//!
//! The guard itself still panics, because an assertion is supposed to. It
//! panics with the typed error's text — which names the path and the cause —
//! rather than with an unqualified `expect`.
//!
//! [`parse_manifest`]: super::parse_manifest

use std::io;

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::DiscoveryError;

/// Reads and parses the manifest at `path`.
///
/// # Errors
///
/// Returns [`DiscoveryError::ManifestUnreadable`] when the path has no parent
/// directory or file name, when its directory cannot be opened, or when the
/// file cannot be read, and [`DiscoveryError::ManifestInvalid`] when the bytes
/// read are not valid TOML. Both name the path they failed on.
pub(crate) fn load_manifest(path: &Utf8Path) -> Result<toml::Table, DiscoveryError> {
    parse_manifest_at(path, &read_manifest(path)?)
}

/// Reads the manifest at `path` as text through a capability-scoped handle.
///
/// The handle is opened over the manifest's own parent directory, so the
/// capability granted is no wider than the file read. That is required by the
/// `no_std_fs_operations` lint, and a test is not an exemption.
fn read_manifest(path: &Utf8Path) -> Result<String, DiscoveryError> {
    let Some(directory) = path.parent() else {
        return Err(invalid(path, "path has no parent directory".to_owned()));
    };
    let Some(filename) = path.file_name() else {
        return Err(invalid(path, "path has no file name".to_owned()));
    };
    let handle = Dir::open_ambient_dir(directory, ambient_authority())
        .map_err(|source| unreadable(path, source))?;
    handle
        .read_to_string(filename)
        .map_err(|source| unreadable(path, source))
}

/// Parses manifest text that was read from `path`.
///
/// The path is carried only so the parse failure can name the file it came
/// from; TOML errors report a line and column but not a file name.
fn parse_manifest_at(path: &Utf8Path, contents: &str) -> Result<toml::Table, DiscoveryError> {
    contents
        .parse::<toml::Table>()
        .map_err(|source| DiscoveryError::ManifestInvalid {
            path: path.to_owned(),
            source,
        })
}

/// Wraps a filesystem failure as an unreadable manifest at `path`.
fn unreadable(path: &Utf8Path, source: io::Error) -> DiscoveryError {
    DiscoveryError::ManifestUnreadable {
        path: path.to_owned(),
        source,
    }
}

/// Reports a manifest path that cannot name a file at all.
///
/// A path with no parent or no final component is a defect in the caller rather
/// than a filesystem refusal, but it is still a failure to read the manifest
/// rather than a panic: the caller holds a `Result` and must be able to report
/// it.
fn invalid(path: &Utf8Path, message: String) -> DiscoveryError {
    unreadable(path, io::Error::new(io::ErrorKind::InvalidInput, message))
}
