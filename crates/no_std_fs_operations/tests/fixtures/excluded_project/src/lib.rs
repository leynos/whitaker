//! Test fixture for excluded crate behaviour.
//!
//! This crate is intentionally a standalone fixture; the duplication with
//! `non_excluded_project` is necessary since each must be an independent crate
//! for exclusion testing.

use std::fs::File;
use std::path::Path;

/// Opens a file for reading.
///
/// # Examples
///
/// ```rust,ignore
/// let file = open_file("Cargo.toml")?;
/// ```
pub fn open_file<P: AsRef<Path>>(path: P) -> std::io::Result<File> {
    File::open(path)
}
