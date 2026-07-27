//! Shared, test-only fixture helpers for `no_std_fs_operations` integration
//! tests.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use tempfile::TempDir;

/// Standalone project fixture created in a temporary directory.
pub(super) struct FixtureProject {
    _temp_dir: TempDir,
    root: PathBuf,
}

impl FixtureProject {
    /// Returns the fixture project root directory.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }
}

/// Creates a temporary fixture project for verifying exclusion behaviour.
///
/// # Examples
///
/// ```ignore
/// let fixture = create_fixture_project("excluded_test_crate", true)?;
/// assert!(fixture.root().join("dylint.toml").exists());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub(super) fn create_fixture_project(
    crate_name: &str,
    is_excluded: bool,
) -> anyhow::Result<FixtureProject> {
    write_fixture_project(
        crate_name,
        &fixture_dylint_config(crate_name, is_excluded),
        &fixture_source(crate_name),
    )
}

/// Creates a fixture whose `std::fs` usage lives inside a nested module.
///
/// When `is_module_excluded` is set, the module's path is listed under
/// `excluded_paths`, exercising the module-path scoped suppression end to end.
///
/// # Examples
///
/// ```ignore
/// let fixture = create_path_fixture_project("path_excluded_crate", true)?;
/// assert!(fixture.root().join("dylint.toml").exists());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub(super) fn create_path_fixture_project(
    crate_name: &str,
    is_module_excluded: bool,
) -> anyhow::Result<FixtureProject> {
    write_fixture_project(
        crate_name,
        &fixture_path_dylint_config(crate_name, is_module_excluded),
        &fixture_module_source(crate_name),
    )
}

/// Scaffolds a standalone fixture crate from a `dylint.toml` and `lib.rs` body.
fn write_fixture_project(
    crate_name: &str,
    dylint_config: &str,
    source: &str,
) -> anyhow::Result<FixtureProject> {
    let temp_dir = TempDir::new().context("failed to create temporary fixture directory")?;
    let root = temp_dir.path().to_path_buf();

    fs::write(
        root.join("Cargo.toml"),
        format!(
            concat!(
                "[package]\n",
                "name = {crate_name}\n",
                "version = \"0.1.0\"\n",
                "edition = \"2024\"\n",
                "\n",
                "[workspace]\n",
                "\n",
                "[dependencies]\n",
            ),
            crate_name = toml::Value::String(crate_name.to_owned())
        ),
    )
    .context("failed to write fixture Cargo.toml")?;

    fs::write(root.join("dylint.toml"), dylint_config)
        .context("failed to write fixture dylint.toml")?;

    let source_dir = root.join("src");
    fs::create_dir(&source_dir).context("failed to create fixture src directory")?;
    fs::write(source_dir.join("lib.rs"), source).context("failed to write fixture source")?;

    Ok(FixtureProject {
        _temp_dir: temp_dir,
        root,
    })
}

fn fixture_dylint_config(crate_name: &str, is_excluded: bool) -> String {
    let excluded_crates = toml::Value::Array(if is_excluded {
        vec![toml::Value::String(crate_name.to_owned())]
    } else {
        Vec::new()
    });

    format!(
        concat!(
            "[no_std_fs_operations]\n",
            "excluded_crates = {excluded_crates}\n",
        ),
        excluded_crates = excluded_crates
    )
}

/// Builds a `dylint.toml` that optionally excludes the fixture's `guarded`
/// module by path (`<crate>::guarded`).
fn fixture_path_dylint_config(crate_name: &str, is_module_excluded: bool) -> String {
    let excluded_paths = toml::Value::Array(if is_module_excluded {
        vec![toml::Value::String(format!("{crate_name}::guarded"))]
    } else {
        Vec::new()
    });

    format!(
        concat!(
            "[no_std_fs_operations]\n",
            "excluded_paths = {excluded_paths}\n",
        ),
        excluded_paths = excluded_paths
    )
}

fn fixture_source(crate_name: &str) -> String {
    format!(
        concat!(
            "//! Temporary fixture crate for `no_std_fs_operations` integration tests.\n",
            "\n",
            "use std::fs::File;\n",
            "use std::path::Path;\n",
            "\n",
            "/// Opens a file for reading.\n",
            "///\n",
            "/// # Examples\n",
            "///\n",
            "/// ```no_run\n",
            "/// use {crate_name}::open_file;\n",
            "///\n",
            "/// let file = open_file(\"Cargo.toml\").expect(\"file should exist\");\n",
            "/// let result = open_file(\"nonexistent.txt\");\n",
            "/// assert!(result.is_err());\n",
            "/// # drop(file);\n",
            "/// ```\n",
            "pub fn open_file<P: AsRef<Path>>(path: P) -> std::io::Result<File> {{\n",
            "    File::open(path)\n",
            "}}\n",
        ),
        crate_name = crate_name
    )
}

/// Emits a fixture whose sole `std::fs` usage lives in a nested `guarded`
/// module, so excluding `<crate>::guarded` should suppress every diagnostic.
///
/// Placing the usage in `guarded::reader` also exercises descendant matching:
/// the configured prefix is the parent module, not the item itself.
///
/// The modules carry inner doc comments so the fixture stays clean under the
/// other Whitaker lints that share the `DYLINT_LIBRARY_PATH` during the run;
/// only `no_std_fs_operations` behaviour is under test here.
fn fixture_module_source(crate_name: &str) -> String {
    // `crate_name` is accepted for parity with the other source generators and
    // to keep call sites uniform, even though this body needs no interpolation.
    let _ = crate_name;
    concat!(
        "//! Temporary fixture crate for `no_std_fs_operations` path exclusion tests.\n",
        "\n",
        "/// Filesystem access confined to an excludable module.\n",
        "pub mod guarded {\n",
        "    //! Module whose path is excluded from `no_std_fs_operations`.\n",
        "\n",
        "    /// Nested module whose ancestor is excluded by path.\n",
        "    pub mod reader {\n",
        "        //! Nested reader confined to the excluded `guarded` ancestor.\n",
        "\n",
        "        use std::fs::File;\n",
        "        use std::path::Path;\n",
        "\n",
        "        /// Opens a file for reading.\n",
        "        pub fn open_file<P: AsRef<Path>>(path: P) -> std::io::Result<File> {\n",
        "            File::open(path)\n",
        "        }\n",
        "    }\n",
        "}\n",
    )
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        create_fixture_project, create_path_fixture_project, fixture_dylint_config,
        fixture_path_dylint_config,
    };

    #[test]
    fn dylint_config_escapes_crate_names_as_toml_values() {
        let crate_name = "crate\"]\ninjected = true\n[other";
        let config = fixture_dylint_config(crate_name, true);
        let parsed: toml::Value = toml::from_str(&config).expect("config should parse as TOML");

        assert_eq!(
            parsed["no_std_fs_operations"]["excluded_crates"][0]
                .as_str()
                .expect("excluded crate should be a string"),
            crate_name
        );
        assert!(parsed.get("other").is_none(), "config was:\n{config}");
        assert!(parsed.get("injected").is_none(), "config was:\n{config}");
    }

    #[test]
    fn fixture_manifest_escapes_crate_names_as_toml_values() -> anyhow::Result<()> {
        let crate_name = "crate\"]\ninjected = true\n[other";
        let fixture = create_fixture_project(crate_name, true)?;
        let manifest = std::fs::read_to_string(fixture.root().join("Cargo.toml"))?;
        let parsed: toml::Value = toml::from_str(&manifest)?;

        assert_eq!(
            parsed["package"]["name"]
                .as_str()
                .expect("package name should be a string"),
            crate_name
        );
        assert!(parsed.get("other").is_none(), "manifest was:\n{manifest}");
        assert!(
            parsed.get("injected").is_none(),
            "manifest was:\n{manifest}"
        );

        Ok(())
    }

    #[test]
    fn path_config_lists_the_guarded_module_when_excluded() {
        let config = fixture_path_dylint_config("my_app", true);
        let parsed: toml::Value = toml::from_str(&config).expect("config should parse as TOML");

        assert_eq!(
            parsed["no_std_fs_operations"]["excluded_paths"][0]
                .as_str()
                .expect("excluded path should be a string"),
            "my_app::guarded"
        );
    }

    #[test]
    fn path_config_omits_exclusions_when_not_excluded() {
        let config = fixture_path_dylint_config("my_app", false);
        let parsed: toml::Value = toml::from_str(&config).expect("config should parse as TOML");

        assert!(
            parsed["no_std_fs_operations"]["excluded_paths"]
                .as_array()
                .expect("excluded_paths should be an array")
                .is_empty()
        );
    }

    #[test]
    fn path_fixture_confines_filesystem_access_to_the_guarded_module() -> anyhow::Result<()> {
        let fixture = create_path_fixture_project("path_fixture_crate", true)?;
        let source = std::fs::read_to_string(fixture.root().join("src/lib.rs"))?;

        // The std::fs usage must sit under the module the config excludes so the
        // integration test genuinely exercises path-scoped suppression.
        assert!(source.contains("pub mod guarded"), "source was:\n{source}");
        assert!(source.contains("std::fs::File"), "source was:\n{source}");
        Ok(())
    }
}
