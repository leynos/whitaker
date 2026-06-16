//! Build-time guards for clone-detector parser metadata.
//!
//! AST hashes are only comparable within one parser schema. This script reads
//! the workspace `ra_ap_syntax` dependency, verifies that it is exact-pinned,
//! and exports `WHITAKER_RA_AP_SYNTAX_VERSION` for `crate::hashing` to mix into
//! every canonical AST hash. Failing the build on a loose parser requirement
//! prevents future dependency updates from silently reusing stale AST hashes.

use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

const PARSER_DEPENDENCY: &str = "ra_ap_syntax";
const PARSER_VERSION_ENV: &str = "WHITAKER_RA_AP_SYNTAX_VERSION";

fn main() -> Result<(), Box<dyn Error>> {
    let workspace_manifest = workspace_manifest_path()?;
    let manifest = fs::read_to_string(&workspace_manifest)?;
    let version_requirement = parser_dependency_requirement(&manifest)?;
    let parser_version = exact_version(&version_requirement)?;

    println!("cargo:rerun-if-changed={}", workspace_manifest.display());
    println!("cargo:rustc-env={PARSER_VERSION_ENV}={parser_version}");

    Ok(())
}

fn workspace_manifest_path() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    find_workspace_manifest(&manifest_dir)
}

fn parser_dependency_requirement(manifest: &str) -> Result<String, Box<dyn Error>> {
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

fn find_workspace_manifest(manifest_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    for directory in manifest_dir.ancestors() {
        let candidate = directory.join("Cargo.toml");
        if candidate.is_file() && is_workspace_manifest(&candidate)? {
            return Ok(candidate);
        }
    }

    Err(workspace_manifest_not_found(manifest_dir).into())
}

fn is_workspace_manifest(candidate: &Path) -> Result<bool, Box<dyn Error>> {
    let manifest = fs::read_to_string(candidate)?;
    let document = manifest.parse::<toml::Table>()?;
    Ok(document.contains_key("workspace"))
}

fn exact_version(requirement: &str) -> Result<&str, io::Error> {
    requirement
        .strip_prefix('=')
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

fn workspace_manifest_not_found(manifest_dir: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "could not find a parent Cargo.toml with a [workspace] table from `{}`",
            manifest_dir.display()
        ),
    )
}
