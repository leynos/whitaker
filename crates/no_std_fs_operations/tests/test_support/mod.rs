//! Shared, test-only fixture helpers for `no_std_fs_operations` integration
//! tests.

use std::{
    fs,
    path::{Path, PathBuf},
};

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

/// Selects which suppression mechanism a fixture exercises.
///
/// The two mechanisms differ in both the `dylint.toml` key they configure and
/// the shape of the generated source, so the fixture builders branch on this.
#[derive(Clone, Copy)]
pub(super) enum FixtureKind {
    /// Crate-wide suppression via `excluded_crates`, with a flat source module.
    CrateExclusion,
    /// Module-path suppression via `excluded_paths`, with `std::fs` usage nested
    /// inside an excludable `guarded` module.
    PathExclusion,
}

impl FixtureKind {
    /// Short label naming the mechanism, used in error context.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::CrateExclusion => "crate",
            Self::PathExclusion => "path",
        }
    }
}

/// Creates a temporary fixture project for verifying exclusion behaviour.
///
/// `kind` selects both the `dylint.toml` configuration and the fixture source:
/// [`FixtureKind::CrateExclusion`] pairs `excluded_crates` with a flat module,
/// while [`FixtureKind::PathExclusion`] pairs `excluded_paths` with `std::fs`
/// usage nested inside a `guarded` module (exercising module-path suppression
/// end to end). `is_excluded` toggles whether the mechanism actually lists the
/// fixture.
///
/// # Examples
///
/// ```ignore
/// let fixture = create_fixture_project(
///     "excluded_test_crate",
///     FixtureKind::CrateExclusion,
///     true,
/// )?;
/// assert!(fixture.root().join("dylint.toml").exists());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub(super) fn create_fixture_project(
    crate_name: &str,
    kind: FixtureKind,
    is_excluded: bool,
) -> anyhow::Result<FixtureProject> {
    let source = match kind {
        FixtureKind::CrateExclusion => fixture_source(crate_name),
        FixtureKind::PathExclusion => fixture_module_source(),
    };
    write_fixture_project(
        crate_name,
        &fixture_dylint_config(crate_name, kind, is_excluded),
        &source,
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

/// Builds a `dylint.toml` for the given suppression mechanism.
///
/// [`FixtureKind::CrateExclusion`] lists `crate_name` under `excluded_crates`;
/// [`FixtureKind::PathExclusion`] lists the fixture's `guarded` module
/// (`<crate>::guarded`) under `excluded_paths`. Entries are serialized through
/// `toml::Value` so crate names remain safely escaped, and an empty array is
/// emitted when `is_excluded` is false.
fn fixture_dylint_config(crate_name: &str, kind: FixtureKind, is_excluded: bool) -> String {
    let (key, entry) = match kind {
        FixtureKind::CrateExclusion => ("excluded_crates", crate_name.to_owned()),
        FixtureKind::PathExclusion => ("excluded_paths", format!("{crate_name}::guarded")),
    };

    let values = toml::Value::Array(if is_excluded {
        vec![toml::Value::String(entry)]
    } else {
        Vec::new()
    });

    format!("[no_std_fs_operations]\n{key} = {values}\n")
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
fn fixture_module_source() -> String {
    // Unlike `fixture_source`, this body needs no crate-name interpolation: the
    // guarded module is a fixed skeleton whose only variable part (the excluded
    // path) lives in the `dylint.toml` produced by `fixture_dylint_config`.
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
    //! Tests for the fixture builders: TOML-safe escaping of crate names in the
    //! generated manifest and `dylint.toml`, and confinement of `std::fs` usage
    //! to the excluded `guarded` module for path-exclusion fixtures.

    use super::{FixtureKind, create_fixture_project, fixture_dylint_config};

    #[test]
    fn dylint_config_escapes_crate_names_as_toml_values() {
        let crate_name = "crate\"]\ninjected = true\n[other";
        let config = fixture_dylint_config(crate_name, FixtureKind::CrateExclusion, true);
        let parsed: toml::Value = toml::from_str(&config).expect("config should parse as TOML");

        let excluded_crate = parsed
            .get("no_std_fs_operations")
            .and_then(|section| section.get("excluded_crates"))
            .and_then(|entries| entries.get(0))
            .and_then(toml::Value::as_str)
            .expect("excluded crate should be a string");
        assert_eq!(excluded_crate, crate_name);
        assert!(parsed.get("other").is_none(), "config was:\n{config}");
        assert!(parsed.get("injected").is_none(), "config was:\n{config}");
    }

    #[test]
    fn fixture_manifest_escapes_crate_names_as_toml_values() -> anyhow::Result<()> {
        let crate_name = "crate\"]\ninjected = true\n[other";
        let fixture = create_fixture_project(crate_name, FixtureKind::CrateExclusion, true)?;
        let manifest = std::fs::read_to_string(fixture.root().join("Cargo.toml"))?;
        let parsed: toml::Value = toml::from_str(&manifest)?;

        let package_name = parsed
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .expect("package name should be a string");
        anyhow::ensure!(
            package_name == crate_name,
            "manifest package name should round-trip, manifest was:\n{manifest}"
        );
        anyhow::ensure!(parsed.get("other").is_none(), "manifest was:\n{manifest}");
        anyhow::ensure!(
            parsed.get("injected").is_none(),
            "manifest was:\n{manifest}"
        );

        Ok(())
    }

    #[test]
    fn path_config_lists_the_guarded_module_when_excluded() {
        let config = fixture_dylint_config("my_app", FixtureKind::PathExclusion, true);
        let parsed: toml::Value = toml::from_str(&config).expect("config should parse as TOML");

        let excluded_path = parsed
            .get("no_std_fs_operations")
            .and_then(|section| section.get("excluded_paths"))
            .and_then(|entries| entries.get(0))
            .and_then(toml::Value::as_str)
            .expect("excluded path should be a string");
        assert_eq!(excluded_path, "my_app::guarded");
    }

    #[test]
    fn path_config_omits_exclusions_when_not_excluded() {
        let config = fixture_dylint_config("my_app", FixtureKind::PathExclusion, false);
        let parsed: toml::Value = toml::from_str(&config).expect("config should parse as TOML");

        let excluded_paths = parsed
            .get("no_std_fs_operations")
            .and_then(|section| section.get("excluded_paths"))
            .and_then(toml::Value::as_array)
            .expect("excluded_paths should be an array");
        assert!(
            excluded_paths.is_empty(),
            "no paths should be excluded, config was:\n{config}"
        );
    }

    #[test]
    fn path_fixture_confines_filesystem_access_to_the_guarded_module() -> anyhow::Result<()> {
        let fixture =
            create_fixture_project("path_fixture_crate", FixtureKind::PathExclusion, true)?;
        let source = std::fs::read_to_string(fixture.root().join("src/lib.rs"))?;

        // The fixture must declare the module the config excludes.
        let Some(guarded_start) = source.find("pub mod guarded") else {
            anyhow::bail!("fixture should declare a guarded module, source was:\n{source}");
        };

        // The `std::fs` usage must be the *only* one, and must sit inside the
        // guarded module. Otherwise a passing exclusion test could reflect an
        // accidental global suppression (or a stray crate-root usage) rather
        // than genuine module-scoped suppression.
        let fs_usages: Vec<usize> = source.match_indices("std::fs").map(|(i, _)| i).collect();
        let [fs_usage] = fs_usages.as_slice() else {
            anyhow::bail!("expected exactly one std::fs usage, source was:\n{source}");
        };
        anyhow::ensure!(
            *fs_usage > guarded_start,
            "the std::fs usage must sit inside the guarded module, source was:\n{source}"
        );
        Ok(())
    }
}
