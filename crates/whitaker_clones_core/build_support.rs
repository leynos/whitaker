//! Pure parser-pin and workspace-manifest discovery shared by build.rs and tests.
//!
//! This module belongs exclusively to build-time validation. Runtime crate
//! code must not import it; integration tests include it only to verify the
//! manifest formats and rejection rules used by `build.rs`.

use std::{error::Error, io};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
const PARSER_DEPENDENCY: &str = "ra_ap_syntax";

pub(crate) fn parser_dependency_requirement(manifest: &str) -> Result<String, Box<dyn Error>> {
    let document = manifest.parse::<toml::Table>()?;
    let dependency = document
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.get(PARSER_DEPENDENCY))
        .ok_or_else(missing_workspace_dependency)?;

    let inline_requirement = dependency.as_str();
    let table_requirement = dependency.get("version").and_then(toml::Value::as_str);

    inline_requirement
        .or(table_requirement)
        .map(str::to_owned)
        .ok_or_else(|| invalid_workspace_dependency().into())
}

pub(crate) fn exact_version(requirement: &str) -> Result<&str, io::Error> {
    requirement
        .strip_prefix('=')
        .filter(|version| !version.is_empty())
        .ok_or_else(|| non_exact_workspace_dependency(requirement))
}

pub(crate) fn find_workspace_manifest(
    manifest_dir: &Utf8Path,
) -> Result<Utf8PathBuf, Box<dyn Error>> {
    for directory in manifest_dir.ancestors() {
        let candidate = directory.join("Cargo.toml");
        if is_workspace_manifest(&candidate)? {
            return Ok(candidate);
        }
    }

    Err(workspace_manifest_not_found(manifest_dir).into())
}

pub(crate) fn is_workspace_manifest(candidate: &Utf8Path) -> Result<bool, Box<dyn Error>> {
    let Some(directory) = candidate.parent() else {
        return Ok(false);
    };
    let Some(filename) = candidate.file_name() else {
        return Ok(false);
    };
    let directory = Dir::open_ambient_dir(directory, ambient_authority())?;
    let manifest = match directory.read_to_string(filename) {
        Ok(manifest) => manifest,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let document = manifest.parse::<toml::Table>()?;
    Ok(document.contains_key("workspace"))
}

fn missing_workspace_dependency() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("workspace dependency `{PARSER_DEPENDENCY}` is missing"),
    )
}

fn invalid_workspace_dependency() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("workspace dependency `{PARSER_DEPENDENCY}` has no version string"),
    )
}

fn non_exact_workspace_dependency(requirement: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "workspace dependency `{PARSER_DEPENDENCY}` must be exact-pinned, got `{requirement}`"
        ),
    )
}

fn workspace_manifest_not_found(manifest_dir: &Utf8Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "could not find a parent Cargo.toml with a [workspace] table from `{}`",
            manifest_dir
        ),
    )
}
