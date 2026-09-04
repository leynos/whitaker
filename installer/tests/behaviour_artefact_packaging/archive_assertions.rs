//! Archive-specific assertions for artefact packaging behaviour tests.

use cap_std::{ambient_authority, fs::Dir};

use super::{PackagingWorld, output_ref};

/// Extract entry names from a `.tar.zst` archive.
pub(super) fn list_archive_entries(world: &PackagingWorld) -> Result<Vec<String>, String> {
    let output = output_ref(world)?;
    let parent = output
        .archive_path
        .parent()
        .ok_or_else(|| String::from("archive path must have a parent directory"))?;
    let name = output
        .archive_path
        .file_name()
        .ok_or_else(|| String::from("archive path must have a file name"))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|error| format!("open archive directory: {error}"))?;
    let file = directory
        .open(name)
        .map_err(|error| format!("open archive: {error}"))?;
    let decoder = zstd::Decoder::new(file).map_err(|error| format!("decode archive: {error}"))?;
    let mut archive = tar::Archive::new(decoder);
    let mut names = Vec::new();
    for entry in archive
        .entries()
        .map_err(|error| format!("list archive entries: {error}"))?
    {
        let archive_entry = entry.map_err(|error| format!("read archive entry: {error}"))?;
        let path = archive_entry
            .path()
            .map_err(|error| format!("read entry path: {error}"))?
            .to_string_lossy()
            .into_owned();
        names.push(path);
    }
    Ok(names)
}
