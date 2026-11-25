//! Whitaker patch: build settings that tests used to put in the environment.
//!
//! Whitaker's UI harness has already built the toolchain-qualified copy of the
//! lint library that Dylint loads, so Dylint must not copy the plain library
//! over it, and on Windows the library build must find vcpkg. The harness used
//! to set `DYLINT_LINK_ENABLE_COPY_LIBRARY` and `VCPKG_ROOT` on the whole test
//! process around every runner. Nothing here changes the process environment:
//! `VCPKG_ROOT` travels on the build command, and the library location Dylint
//! derives from the copy-library flag is computed directly.

use std::{
    env::current_dir,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use anyhow::Result;
use dylint_internal::{env, library_filename_with_toolchain, link::dylint_libs};

/// The Dylint flag that says the toolchain-qualified library is prepared.
///
/// Since Dylint 6.3.0, `dylint_libs` copies the plain library over the prepared
/// one unless this flag is on. For every Whitaker lint crate the plain library
/// is an empty `cdylib`, so copying it makes the UI test fail with
/// ``could not find `dylint_version` ``.
const COPY_LIBRARY_FLAG: &str = env::DYLINT_LINK_ENABLE_COPY_LIBRARY;

/// Where Windows CI installs vcpkg.
const VCPKG_DIRECTORY: &str = r"C:\vcpkg";

/// Report whether the toolchain-qualified library should be treated as prepared.
///
/// An absent flag means prepared, because Whitaker's harness prepares it; a
/// caller or CI that sets the flag to `0` asks Dylint to copy the library, and
/// any other value says it is prepared.
pub(crate) fn copy_is_prepared(flag: Option<OsString>) -> bool {
    flag.is_none_or(|value| value != "0")
}

/// Select `VCPKG_ROOT` for the library build when it is needed and not set.
///
/// A value the caller or CI supplied is left in place: the build command
/// inherits it, and passing a default here would override it.
pub(crate) fn vcpkg_root(
    is_set: impl Fn(&OsStr) -> bool,
    vcpkg_installed: bool,
) -> Option<(&'static str, &'static str)> {
    (cfg!(windows) && vcpkg_installed && !is_set(OsStr::new("VCPKG_ROOT")))
        .then_some(("VCPKG_ROOT", VCPKG_DIRECTORY))
}

/// Whether the Windows vcpkg directory exists.
pub(crate) fn vcpkg_directory_exists() -> bool { Path::new(VCPKG_DIRECTORY).is_dir() }

/// The `DYLINT_LIBS` value for the library under test, as JSON.
///
/// When the copy-library flag marks the library as prepared (see
/// [`copy_is_prepared`]) the answer is the toolchain-qualified file in Cargo's
/// debug directory, which is what Dylint computes for an enabled flag. When the
/// caller turned the flag off, Dylint's own function runs and copies the plain
/// library as it always did.
pub(crate) fn prepared_dylint_libs(name: &str, toolchain: &str) -> Result<String> {
    prepared_dylint_libs_given(std::env::var_os(COPY_LIBRARY_FLAG), name, toolchain)
}

/// [`prepared_dylint_libs`] with the flag's value supplied by the caller.
fn prepared_dylint_libs_given(
    flag: Option<OsString>,
    name: &str,
    toolchain: &str,
) -> Result<String> {
    if !copy_is_prepared(flag) {
        return dylint_libs(name, toolchain);
    }
    let metadata = dylint_internal::cargo::metadata(current_dir()?)?;
    let debug_directory = metadata
        .build_directory
        .as_ref()
        .unwrap_or(&metadata.target_directory)
        .join("debug");
    let library: PathBuf = debug_directory
        .join(library_filename_with_toolchain(name, toolchain))
        .into_std_path_buf();
    serde_json::to_string(&[library]).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_copy_flag_counts_as_prepared() {
        assert!(copy_is_prepared(None));
    }

    #[test]
    fn a_flag_of_zero_asks_dylint_to_copy() {
        assert!(!copy_is_prepared(Some(OsString::from("0"))));
    }

    #[test]
    fn any_other_flag_value_counts_as_prepared() {
        assert!(copy_is_prepared(Some(OsString::from("1"))));
    }

    /// With no flag the answer is the qualified library in Cargo's debug
    /// directory, as a one-element JSON list, and nothing is copied.
    #[test]
    fn a_prepared_library_is_located_without_copying() {
        let json = prepared_dylint_libs_given(None, "example_lint", "nightly-2026-01-01")
            .expect("the library location should be computed");

        let paths: Vec<PathBuf> =
            serde_json::from_str(&json).expect("the location should be a JSON list of paths");
        let [path] = paths.as_slice() else {
            panic!("expected exactly one library path, got {paths:?}");
        };
        assert_eq!(
            path.file_name().and_then(OsStr::to_str),
            Some(library_filename_with_toolchain("example_lint", "nightly-2026-01-01").as_str()),
            "the path should name the toolchain-qualified library"
        );
        assert!(
            path.parent()
                .is_some_and(|parent| parent.ends_with("debug")),
            "the library should sit in Cargo's debug directory: {}",
            path.display()
        );
        assert!(!path.exists(), "locating the library must not create it");
    }

    /// Marks the re-executed test binary as the child of
    /// [`a_flag_of_zero_copies_the_plain_library_in_a_child_process`].
    const CHILD_MARKER: &str = "WHITAKER_BUILD_ENV_COPY_CHILD";
    const CHILD_LIBRARY: &str = "example_lint";
    const CHILD_TOOLCHAIN: &str = "nightly-2026-01-01";

    /// With the flag at `0`, Dylint's own function runs and copies the plain
    /// library to its toolchain-qualified name. The copy resolves the Cargo
    /// package from the working directory, so the check runs in a child
    /// process started inside a scratch package; this process's environment
    /// and working directory are never touched.
    #[test]
    fn a_flag_of_zero_copies_the_plain_library_in_a_child_process() {
        if std::env::var_os(CHILD_MARKER).is_some() {
            prepared_dylint_libs(CHILD_LIBRARY, CHILD_TOOLCHAIN)
                .expect("the fallback should copy the plain library");
            return;
        }
        let package = tempfile::tempdir().expect("a scratch package directory");
        let root = package.path();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"example_lint\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        )
        .expect("write the manifest");
        std::fs::create_dir_all(root.join("src")).expect("create src");
        std::fs::write(root.join("src/lib.rs"), "").expect("write the library root");
        let debug = root.join("target").join("debug");
        std::fs::create_dir_all(&debug).expect("create the debug directory");
        std::fs::write(
            debug.join(dylint_internal::library_plain_filename(CHILD_LIBRARY)),
            b"plain",
        )
        .expect("write the plain library");

        let output = std::process::Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                "--exact",
                "build_env::tests::a_flag_of_zero_copies_the_plain_library_in_a_child_process",
                "--nocapture",
            ])
            .current_dir(root)
            .env(CHILD_MARKER, "1")
            .env(COPY_LIBRARY_FLAG, "0")
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("CARGO_BUILD_TARGET_DIR")
            .env_remove("CARGO_BUILD_BUILD_DIR")
            .output()
            .expect("the child process should start");

        assert!(
            output.status.success(),
            "the child should succeed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let copy = debug.join(library_filename_with_toolchain(
            CHILD_LIBRARY,
            CHILD_TOOLCHAIN,
        ));
        assert!(
            copy.is_file(),
            "a flag of 0 should copy the plain library to {}",
            copy.display()
        );
    }

    #[test]
    fn vcpkg_is_supplied_only_on_windows_when_installed_and_absent() {
        let installed = vcpkg_root(|_| false, true);
        let missing = vcpkg_root(|_| false, false);
        let present = vcpkg_root(|key| key == "VCPKG_ROOT", true);

        assert_eq!(installed.is_some(), cfg!(windows));
        assert_eq!(missing, None);
        assert_eq!(present, None);
    }
}
