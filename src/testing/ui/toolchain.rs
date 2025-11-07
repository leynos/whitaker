//! Toolchain preparation helpers for the Whitaker UI harness.
//!
//! The harness needs the compiled lint library to exist under a
//! toolchain-qualified filename so Dylint discovers the latest build each run.
//! This module owns the Cargo metadata queries and artefact management needed
//! to refresh that copy.

use std::{
    env, fmt, fs,
    io::Cursor,
    path::PathBuf,
    process::{Command, Output},
};

use cargo_metadata::{self, Message, Metadata, MetadataCommand};

use super::HarnessError;

#[derive(Debug, Clone)]
pub(super) struct CrateName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CrateNameError;

impl CrateName {
    fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<&str> for CrateName {
    type Error = CrateNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(CrateNameError)
        } else {
            Ok(Self::new(value))
        }
    }
}

impl TryFrom<String> for CrateName {
    type Error = CrateNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(CrateNameError)
        } else {
            Ok(Self(value))
        }
    }
}

impl AsRef<str> for CrateName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub(super) fn ensure_toolchain_library(crate_name: &CrateName) -> Result<(), HarnessError> {
    let metadata = fetch_metadata()?;

    if !workspace_has_package(&metadata, crate_name) {
        // The harness is being exercised with a synthetic crate name. In that case the caller
        // controls the build and we should not attempt to prepare artefacts.
        return Ok(());
    }

    let source = build_and_locate_cdylib(crate_name, &metadata)?;
    let parent = source
        .parent()
        .ok_or_else(|| HarnessError::LibraryMissing {
            path: source.display().to_string(),
        })?;

    let toolchain = env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .unwrap_or_else(|| "unknown-toolchain".to_owned());
    let file_name = source
        .file_name()
        .ok_or_else(|| HarnessError::LibraryMissing {
            path: source.display().to_string(),
        })?
        .to_string_lossy()
        .into_owned();
    let target_name = build_target_name(&file_name, &toolchain);
    let target = parent.join(&target_name);

    // Always refresh the toolchain-qualified artefact so UI tests exercise the latest build.
    fs::copy(&source, &target).map_err(|error| HarnessError::LibraryCopyFailed {
        source: source.display().to_string(),
        target: target.display().to_string(),
        message: error.to_string(),
    })?;

    Ok(())
}

fn build_target_name(file_name: &str, toolchain: &str) -> String {
    let suffix = env::consts::DLL_SUFFIX;
    file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    )
}

fn build_and_locate_cdylib(
    crate_name: &CrateName,
    metadata: &Metadata,
) -> Result<PathBuf, HarnessError> {
    let output = execute_build_command(crate_name, metadata)?;
    let package_id = find_package_id(crate_name, metadata)?;
    find_cdylib_in_artifacts(&output.stdout, &package_id, crate_name)
}

fn execute_build_command(
    crate_name: &CrateName,
    metadata: &Metadata,
) -> Result<Output, HarnessError> {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--lib")
        .arg("--quiet")
        .arg("--message-format=json")
        .arg("--package")
        .arg(crate_name.as_str())
        .current_dir(metadata.workspace_root.as_std_path());

    let output = command
        .output()
        .map_err(|error| HarnessError::LibraryBuildFailed {
            crate_name: crate_name.as_str().to_owned(),
            message: error.to_string(),
        })?;

    if !output.status.success() {
        return Err(HarnessError::LibraryBuildFailed {
            crate_name: crate_name.as_str().to_owned(),
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(output)
}

fn find_package_id(
    crate_name: &CrateName,
    metadata: &Metadata,
) -> Result<cargo_metadata::PackageId, HarnessError> {
    metadata
        .packages
        .iter()
        .find(|package| {
            package.name == crate_name.as_str()
                && metadata
                    .workspace_members
                    .iter()
                    .any(|member| member == &package.id)
        })
        .map(|package| package.id.clone())
        .ok_or_else(|| HarnessError::LibraryBuildFailed {
            crate_name: crate_name.as_str().to_owned(),
            message: format!(
                "package metadata missing for {crate_name}; unable to locate cdylib artefact"
            ),
        })
}

fn find_cdylib_in_artifacts(
    stdout: &[u8],
    package_id: &cargo_metadata::PackageId,
    crate_name: &CrateName,
) -> Result<PathBuf, HarnessError> {
    for message in Message::parse_stream(Cursor::new(stdout)) {
        let Ok(Message::CompilerArtifact(artifact)) = message else {
            // Ignore unrelated output and parse errors; the build succeeded so any
            // remaining noise should not block locating the compiled artefact.
            continue;
        };

        if let Some(path) = extract_cdylib_path(&artifact, package_id) {
            return Ok(path);
        }
    }

    Err(HarnessError::LibraryMissing {
        path: format!("cdylib for {crate_name} not reported by cargo"),
    })
}

fn extract_cdylib_path(
    artifact: &cargo_metadata::Artifact,
    package_id: &cargo_metadata::PackageId,
) -> Option<PathBuf> {
    if artifact.package_id != *package_id {
        return None;
    }

    if !artifact.target.is_cdylib() {
        return None;
    }

    artifact
        .filenames
        .iter()
        .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        .map(|path| path.clone().into_std_path_buf())
}

fn fetch_metadata() -> Result<Metadata, HarnessError> {
    MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|error| HarnessError::MetadataFailed {
            message: error.to_string(),
        })
}

fn workspace_has_package(metadata: &Metadata, crate_name: &CrateName) -> bool {
    metadata
        .packages
        .iter()
        .any(|package| is_workspace_member(metadata, package, crate_name))
}

fn is_workspace_member(
    metadata: &Metadata,
    package: &cargo_metadata::Package,
    crate_name: &CrateName,
) -> bool {
    package.name == crate_name.as_str()
        && metadata
            .workspace_members
            .iter()
            .any(|member| member == &package.id)
}
