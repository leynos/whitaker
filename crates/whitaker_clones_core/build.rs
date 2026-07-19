//! Build-time guards for clone-detector parser metadata.
//!
//! AST hashes are only comparable within one parser schema. This script reads
//! the workspace `ra_ap_syntax` dependency, verifies that it is exact-pinned,
//! and exports `WHITAKER_RA_AP_SYNTAX_VERSION` for `crate::hashing` to mix into
//! every canonical AST hash. Failing the build on a loose parser requirement
//! prevents future dependency updates from silently reusing stale AST hashes.

use std::{env, error::Error, fs};

use camino::Utf8PathBuf;
mod build_support;

use build_support::{exact_version, find_workspace_manifest, parser_dependency_requirement};

const PARSER_VERSION_ENV: &str = "WHITAKER_RA_AP_SYNTAX_VERSION";

fn main() -> Result<(), Box<dyn Error>> {
    let workspace_manifest = workspace_manifest_path()?;
    let manifest = fs::read_to_string(&workspace_manifest)?;
    let version_requirement = parser_dependency_requirement(&manifest)?;
    let parser_version = exact_version(&version_requirement)?;

    println!("cargo:rerun-if-changed={workspace_manifest}");
    println!("cargo:rustc-env={PARSER_VERSION_ENV}={parser_version}");

    Ok(())
}

fn workspace_manifest_path() -> Result<Utf8PathBuf, Box<dyn Error>> {
    let manifest_dir = Utf8PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    find_workspace_manifest(&manifest_dir)
}
