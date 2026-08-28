//! Archive-specific assertions for artefact packaging behaviour tests.

use std::fs;

use super::{PackagingWorld, output_ref};

/// Extract entry names from a `.tar.zst` archive.
pub(super) fn list_archive_entries(world: &PackagingWorld) -> Result<Vec<String>, String> {
    let output = output_ref(world)?;
    let file =
        fs::File::open(&output.archive_path).map_err(|error| format!("open archive: {error}"))?;
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
