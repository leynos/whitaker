//! Pure parser-pin extraction shared by the build script and its tests.
//!
//! This module belongs exclusively to build-time validation. Runtime crate
//! code must not import it; integration tests include it only to verify the
//! manifest formats and rejection rules used by `build.rs`.

use std::{error::Error, io};

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
