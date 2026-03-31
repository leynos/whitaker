//! Target and filename helpers for dependency-binary installation.

use crate::artefact::target::TargetTriple;

use super::super::manifest::DependencyBinary;

const PROVENANCE_FILENAME: &str = "dependency-binaries-licences.md";

/// Return the current host target when Whitaker knows how to package it.
#[must_use]
pub fn host_target() -> Option<TargetTriple> {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    let target = "x86_64-unknown-linux-gnu";
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    let target = "aarch64-unknown-linux-gnu";
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    let target = "x86_64-apple-darwin";
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    let target = "aarch64-apple-darwin";
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    let target = "x86_64-pc-windows-msvc";
    #[cfg(not(any(
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "linux"),
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "windows"),
    )))]
    let target = "";

    if target.is_empty() {
        None
    } else {
        TargetTriple::try_from(target).ok()
    }
}

/// Return the release-side provenance asset filename.
#[must_use]
pub fn provenance_filename() -> &'static str {
    PROVENANCE_FILENAME
}

/// Compute the platform-specific executable name for a dependency binary.
#[must_use]
pub fn binary_filename(dependency: &DependencyBinary, target: &TargetTriple) -> String {
    if target.is_windows() {
        format!("{}.exe", dependency.binary())
    } else {
        dependency.binary().to_owned()
    }
}

/// Compute the repository archive filename for a dependency binary.
#[must_use]
pub fn archive_filename(dependency: &DependencyBinary, target: &TargetTriple) -> String {
    let extension = if target.is_windows() { "zip" } else { "tgz" };
    format!(
        "{}-{}-v{}.{}",
        dependency.package(),
        target.as_str(),
        dependency.version(),
        extension
    )
}

/// Compute the exact archive member path that should contain the executable.
#[must_use]
pub(crate) fn expected_member_path(dependency: &DependencyBinary, target: &TargetTriple) -> String {
    format!(
        "{}-{}-v{}/{}",
        dependency.package(),
        target.as_str(),
        dependency.version(),
        binary_filename(dependency, target)
    )
}
