//! Regression coverage for crate-relative localization packaging.
//!
//! The `whitaker-common` crate is published independently, so its Fluent
//! bundles must be present in the packaged tarball rather than only in the
//! checkout.

#[cfg(unix)]
mod unix {
    //! Unix-only packaging checks that shell out to `cargo package` and `tar`.
    //!
    //! These helpers stage a temporary target directory, build the package
    //! tarball, and list its contents so the test can assert that the
    //! fallback Fluent bundle ships with the crate.

    use std::{
        error::Error,
        fs, io,
        path::{Path, PathBuf},
        process::Command,
    };

    use tempfile::{Builder, TempDir};
    use whitaker_common::i18n::packaged_fallback_locale_path;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    #[test]
    fn fluent_bundles_are_included_in_the_package_tarball() -> TestResult {
        let target_dir = package_target_dir()?;
        let crate_path = package_crate_path(target_dir.path())?;
        let tar_listing = package_tar_listing(&crate_path)?;
        let expected_entry = packaged_fallback_locale_path()
            .to_string_lossy()
            .replace('\\', "/");

        if tar_listing.lines().any(|line| line == expected_entry) {
            Ok(())
        } else {
            Err(format!(
                "expected packaged tarball to include the fallback Fluent bundle \
                 `{expected_entry}`, but it did not"
            )
            .into())
        }
    }

    fn package_target_dir() -> io::Result<TempDir> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target_root = manifest_dir.join("target");
        fs::create_dir_all(&target_root)?;

        Builder::new()
            .prefix("whitaker-common-package-")
            .tempdir_in(&target_root)
    }

    fn package_crate_path(target_dir: &Path) -> TestResult<PathBuf> {
        let status = Command::new("cargo")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("CARGO_TARGET_DIR", target_dir)
            .args([
                "package",
                "-p",
                "whitaker-common",
                "--allow-dirty",
                "--no-verify",
            ])
            .status()?;

        if !status.success() {
            return Err(format!("cargo package should succeed, but exited with {status}").into());
        }

        let expected_name = format!(
            "{}-{}.crate",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        );
        let package_dir = target_dir.join("package");
        for entry_result in fs::read_dir(&package_dir)? {
            let path = entry_result?.path();
            if path
                .file_name()
                .is_some_and(|name| name == expected_name.as_str())
            {
                return Ok(path);
            }
        }

        Err(format!("cargo package should produce {expected_name}").into())
    }

    fn package_tar_listing(crate_path: &Path) -> TestResult<String> {
        let output = Command::new("tar").arg("-tf").arg(crate_path).output()?;

        if !output.status.success() {
            return Err(format!(
                "tar should succeed when listing the packaged crate: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        String::from_utf8(output.stdout).map_err(Into::into)
    }
}
