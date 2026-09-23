//! Capability-scoped fixture filesystem operations for prebuilt scenarios.

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Writes a fixture file through a capability scoped to its parent directory.
pub(super) fn write_file(path: &Utf8Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("fixture path must have a parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("fixture path must have a file name"))?;
    Dir::open_ambient_dir(parent, ambient_authority())?.write(name, contents)
}
