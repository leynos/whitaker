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

mod build_support;

use build_support::{exact_version, parser_dependency_requirement};

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

fn workspace_manifest_not_found(manifest_dir: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "could not find a parent Cargo.toml with a [workspace] table from `{}`",
            manifest_dir.display()
        ),
    )
}
