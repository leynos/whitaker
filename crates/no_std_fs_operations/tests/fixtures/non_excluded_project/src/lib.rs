//! Test fixture for non-excluded crate behaviour.
//!
//! This crate is intentionally a standalone fixture; the duplication with
//! `excluded_project` is necessary since each must be an independent crate
//! for exclusion testing.

use std::fs::File;
use std::path::Path;

/// Opens a file for reading.
///
/// # Examples
///
/// ```rust,ignore
/// // Open existing file - returns Ok with file handle
/// let file = open_file("Cargo.toml").expect("file should exist");
///
/// // Attempt to open non-existent file - returns Err
/// let result = open_file("nonexistent.txt");
/// assert!(result.is_err());
/// ```
pub fn open_file<P: AsRef<Path>>(path: P) -> std::io::Result<File> {
    File::open(path)
}
