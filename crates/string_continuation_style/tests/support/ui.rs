//! Runs lint UI fixtures under each supported locale.

use std::io::{self, Write};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::OpenOptions, fs_utf8::Dir};
use serial_test::serial;
use whitaker_common::test_support::with_locale;

#[test]
#[serial]
fn ui() { run_with_locale("ui", None); }

#[test]
#[serial]
fn ui_cy() { run_with_locale("ui-cy", Some("cy")); }

#[test]
#[serial]
fn ui_gd() { run_with_locale("ui-gd", Some("gd")); }

#[test]
#[serial]
fn ui_fallback() { run_with_locale("ui-fallback", Some("zz")); }

fn run_with_locale(directory: &str, locale: Option<&str>) {
    let _generated_source = GeneratedSource::create()
        .expect("generated-source UI input should be prepared in the target directory");
    with_locale(locale, || run_ui_directory(directory));
}

fn run_ui_directory(directory: &str) {
    // Rustfix only applies machine-applicable suggestions, while the shipped
    // default remains manual until this harness proves the fixes end to end.
    whitaker::testing::ui::run_with_runner(
        env!("CARGO_PKG_NAME"),
        directory,
        |crate_name, directory| {
            let mut test = dylint_testing::ui::Test::src_base(crate_name, directory);
            test.dylint_toml("[string_continuation_style]\napplicability = \"machine\"\n");
            test.run();
            Ok(())
        },
    )
    .expect("UI tests should execute without diffs");
}

struct GeneratedSource {
    target_directory: Dir,
    path: Utf8PathBuf,
}

impl GeneratedSource {
    fn create() -> io::Result<Self> {
        let target_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
        let target_directory = Dir::open_ambient_dir(&target_root, ambient_authority())?;
        let path = Utf8PathBuf::from("string_continuation_style-generated-ui.rs");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = target_directory.open_with(&path, &options)?;
        if let Err(error) = file.write_all(
            br#"const GENERATED: &str = "alpha \
    beta";
"#,
        ) {
            drop(file);
            let _ = target_directory.remove_file(&path);
            return Err(error);
        }
        Ok(Self {
            target_directory,
            path,
        })
    }
}

impl Drop for GeneratedSource {
    fn drop(&mut self) { let _ = self.target_directory.remove_file(&self.path); }
}
