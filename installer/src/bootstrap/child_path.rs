//! The `PATH` the child `cargo dylint` runs with.
//!
//! Dylint reuses a persisted driver only if `dylint-driver -V` succeeds, and it
//! rebuilds the driver otherwise, silently. On Windows the driver loads the
//! pinned toolchain's compiler DLLs from `PATH`, and the child's `PATH` names
//! the toolchain the user's `cargo` proxy chose, not the pinned one. The probe
//! then dies with `STATUS_DLL_NOT_FOUND` (0xC0000135), so every warm run
//! rebuilt the driver for about two minutes. Appending the pinned toolchain's
//! `bin` directory lets the DLLs load. It goes last so that it never shadows
//! a tool the user's own toolchain provides.

use std::ffi::OsString;
#[cfg(any(windows, test))]
use std::{env, ffi::OsStr, path::PathBuf};

/// Return `current` with `dir` appended as its last entry.
///
/// An absent or empty `current` yields just `dir`.
#[cfg(any(windows, test))]
pub(super) fn append_dir(current: Option<&OsStr>, dir: PathBuf) -> OsString {
    let mut entries: Vec<PathBuf> = current
        .map(|value| env::split_paths(value).collect())
        .unwrap_or_default();
    entries.retain(|entry| !entry.as_os_str().is_empty());
    entries.push(dir);
    env::join_paths(entries).unwrap_or_default()
}

/// Return the child's `PATH` with the pinned toolchain's `bin` appended.
///
/// `None` means "leave `PATH` alone": on every platform but Windows, and on
/// Windows when the toolchain's sysroot cannot be resolved, in which case the
/// run behaves as it did before.
#[cfg(windows)]
pub(super) fn child_path(toolchain: &str) -> Option<OsString> {
    let output = std::process::Command::new("rustc")
        .arg(format!("+{toolchain}"))
        .args(["--print", "sysroot"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let sysroot = String::from_utf8(output.stdout).ok()?;
    let bin = PathBuf::from(sysroot.trim()).join("bin");
    Some(append_dir(env::var_os("PATH").as_deref(), bin))
}

/// Return the child's `PATH`; only Windows needs a change, so never here.
#[cfg(not(windows))]
pub(super) const fn child_path(_toolchain: &str) -> Option<OsString> { None }

#[cfg(test)]
mod tests {
    //! Unit tests for the `PATH` arithmetic.

    use std::{env, ffi::OsString, path::PathBuf};

    use rstest::rstest;

    use super::append_dir;

    fn entries(value: &OsString) -> Vec<PathBuf> { env::split_paths(value).collect() }

    #[rstest]
    fn appends_the_directory_last() {
        let current = env::join_paths(["first", "second"]).expect("join");
        let joined = append_dir(Some(current.as_os_str()), PathBuf::from("toolchain-bin"));
        assert_eq!(
            entries(&joined),
            ["first", "second", "toolchain-bin"].map(PathBuf::from)
        );
    }

    #[rstest]
    fn an_absent_path_yields_only_the_directory() {
        let joined = append_dir(None, PathBuf::from("toolchain-bin"));
        assert_eq!(entries(&joined), [PathBuf::from("toolchain-bin")]);
    }

    #[rstest]
    fn empty_entries_are_dropped() {
        let current = OsString::new();
        let joined = append_dir(Some(current.as_os_str()), PathBuf::from("toolchain-bin"));
        assert_eq!(entries(&joined), [PathBuf::from("toolchain-bin")]);
    }
}
