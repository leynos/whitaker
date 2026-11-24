//! Whitaker patch: variables the library build and lookup need when tests
//! start them.
//!
//! `initialize` builds the lint library under test and then asks Dylint where
//! the toolchain-qualified copy lives. Whitaker's UI harness has already built
//! that copy, so Dylint must not overwrite it, and on Windows the build must
//! find vcpkg. The harness used to set these variables around every runner;
//! `initialize` now sets the absent ones once, beside the driver variables it
//! already sets, so tests never touch the process environment themselves.

use std::{ffi::OsStr, path::Path};

/// Tells Dylint 6.3.0's `dylint_libs` that the toolchain-qualified library is
/// prepared, so it does not copy the plain library (an empty `cdylib` for every
/// Whitaker lint crate) over it. Without it the UI test loads that stub and
/// fails with ``could not find `dylint_version` ``.
const COPY_LIBRARY: (&str, &str) = ("DYLINT_LINK_ENABLE_COPY_LIBRARY", "1");

/// Where Windows CI installs vcpkg.
const VCPKG_DIRECTORY: &str = r"C:\vcpkg";

/// Select the variables the process does not already set.
///
/// A value the caller or CI supplied is left in place, so a deliberate setting
/// is never replaced.
pub(crate) fn defaults(
    is_set: impl Fn(&OsStr) -> bool,
    vcpkg_installed: bool,
) -> Vec<(&'static str, &'static str)> {
    let mut wanted = vec![COPY_LIBRARY];
    if cfg!(windows) && vcpkg_installed {
        wanted.push(("VCPKG_ROOT", VCPKG_DIRECTORY));
    }
    wanted
        .into_iter()
        .filter(|(key, _)| !is_set(OsStr::new(key)))
        .collect()
}

/// Whether the Windows vcpkg directory exists.
pub(crate) fn vcpkg_directory_exists() -> bool { Path::new(VCPKG_DIRECTORY).is_dir() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_copy_flag_is_supplied() {
        assert!(defaults(|_| false, false).contains(&COPY_LIBRARY));
    }

    #[test]
    fn a_caller_supplied_copy_flag_is_not_overridden() {
        let selected = defaults(|key| key == COPY_LIBRARY.0, false);

        assert!(!selected.iter().any(|(key, _)| *key == COPY_LIBRARY.0));
    }

    #[test]
    fn vcpkg_is_supplied_only_on_windows_when_installed_and_absent() {
        let installed = defaults(|_| false, true);
        let missing = defaults(|_| false, false);
        let present = defaults(|key| key == "VCPKG_ROOT", true);

        let has_vcpkg = |list: &[(&str, &str)]| list.iter().any(|(key, _)| *key == "VCPKG_ROOT");
        assert_eq!(has_vcpkg(&installed), cfg!(windows));
        assert!(!has_vcpkg(&missing));
        assert!(!has_vcpkg(&present));
    }
}
