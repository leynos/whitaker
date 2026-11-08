//! Runs lint UI fixtures under each supported locale.

use std::io::{self, Write};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::OpenOptions, fs_utf8::Dir};
use serial_test::serial;
use whitaker_common::test_support::with_locale;

#[test]
#[serial]
fn ui() {
    // Only the `ui/` fixture reads the generated file, so only this test needs
    // it. The other three locale runs would otherwise create and delete the
    // same path for no reason.
    let _generated_source = GeneratedSource::create()
        .expect("generated-source UI input should be prepared in the target directory");
    run_with_locale("ui", None).expect("UI tests should execute without diffs");
}

#[test]
#[serial]
fn ui_cy() { run_with_locale("ui-cy", Some("cy")).expect("UI tests should execute without diffs"); }

#[test]
#[serial]
fn ui_gd() { run_with_locale("ui-gd", Some("gd")).expect("UI tests should execute without diffs"); }

#[test]
#[serial]
fn ui_fallback() {
    run_with_locale("ui-fallback", Some("zz")).expect("UI tests should execute without diffs");
}

fn run_with_locale(
    directory: &str,
    locale: Option<&str>,
) -> Result<(), whitaker::testing::ui::HarnessError> {
    with_locale(locale, || run_ui_directory(directory))
}

fn run_ui_directory(directory: &str) -> Result<(), whitaker::testing::ui::HarnessError> {
    // Rustfix only applies machine-applicable suggestions, while the shipped
    // default remains manual until this harness proves the fixes end to end.
    whitaker::testing::ui::run_with_runner(
        env!("CARGO_PKG_NAME"),
        directory,
        |crate_name, fixture_directory| {
            let mut test = dylint_testing::ui::Test::src_base(crate_name, fixture_directory);
            test.dylint_toml("[string_continuation_style]\napplicability = \"machine\"\n");
            test.run();
            Ok(())
        },
    )
}

struct GeneratedSource {
    target_directory: Dir,
    path: Utf8PathBuf,
}

impl GeneratedSource {
    fn create() -> io::Result<Self> {
        let target_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
        // `CARGO_TARGET_DIR` can point elsewhere and the directory may not have
        // been created yet, so create the fixed fixture location rather than
        // assuming the default `target/` already exists.
        Dir::create_ambient_dir_all(&target_root, ambient_authority())?;
        let target_directory = Dir::open_ambient_dir(&target_root, ambient_authority())?;
        let path = Utf8PathBuf::from("string_continuation_style-generated-ui.rs");
        // An interrupted earlier run leaves the file behind, and `create_new`
        // would then fail for the rest of the session. Remove it first, but
        // ignore "not found" so a clean target directory still succeeds.
        match target_directory.remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = target_directory.open_with(&path, &options)?;
        if let Err(error) = file.write_all(
            br#"const GENERATED: &str = "alpha \
    beta";
"#,
        ) {
            drop(file);
            drop(target_directory.remove_file(&path));
            return Err(error);
        }
        Ok(Self {
            target_directory,
            path,
        })
    }
}

impl Drop for GeneratedSource {
    fn drop(&mut self) { drop(self.target_directory.remove_file(&self.path)); }
}
