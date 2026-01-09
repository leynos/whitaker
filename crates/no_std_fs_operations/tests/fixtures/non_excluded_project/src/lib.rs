//! Test fixture for non-excluded crate behaviour.

use std::fs::File;

/// Opens a file for reading.
pub fn open_file(path: &str) -> std::io::Result<File> {
    File::open(path)
}
