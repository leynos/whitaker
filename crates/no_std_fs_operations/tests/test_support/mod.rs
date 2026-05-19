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
                "[dependencies]\n",
            ),
            crate_name = toml::Value::String(crate_name.to_owned())
        ),
    )
    .context("failed to write fixture Cargo.toml")?;

    fs::write(
        root.join("dylint.toml"),
        fixture_dylint_config(crate_name, is_excluded),
    )
    .context("failed to write fixture dylint.toml")?;

    let source_dir = root.join("src");
    fs::create_dir(&source_dir).context("failed to create fixture src directory")?;
    fs::write(source_dir.join("lib.rs"), fixture_source(crate_name))
        .context("failed to write fixture source")?;

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

#[cfg(test)]
mod tests {
    use super::{create_fixture_project, fixture_dylint_config};

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
}
