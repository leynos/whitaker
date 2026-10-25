//! This crate provides convenient access to the [`compiletest_rs`] package for testing [Dylint]
//! libraries.
//!
//! **Note: If your test has dependencies, you must use `ui_test_example` or `ui_test_examples`.**
//! See the [`question_mark_in_expression`] example in this repository.
//!
//! This crate provides the following three functions:
//!
//! - [`ui_test`] - test a library on all source files in a directory
//! - [`ui_test_example`] - test a library on one example target
//! - [`ui_test_examples`] - test a library on all example targets
//!
//! For most situations, you can add the following to your library's `lib.rs` file:
//!
//! ```rust,ignore
//! #[test]
//! fn ui() {
//!     dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
//! }
//! ```
//!
//! And include one or more `.rs` and `.stderr` files in a `ui` directory alongside your library's
//! `src` directory. See the [examples] in this repository.
//!
//! # Test builder
//!
//! In addition to the above three functions, [`ui::Test`] is a test "builder." Currently, the main
//! advantage of using `Test` over the above functions is that `Test` allows flags to be passed to
//! `rustc`. For an example of its use, see [`non_thread_safe_call_in_test`] in this repository.
//!
//! `Test` has three constructors, which correspond to the above three functions as follows:
//!
//! - [`ui::Test::src_base`] <-> [`ui_test`]
//! - [`ui::Test::example`] <-> [`ui_test_example`]
//! - [`ui::Test::examples`] <-> [`ui_test_examples`]
//!
//! In each case, the constructor's arguments are exactly those of the corresponding function.
//!
//! A `Test` instance has the following methods:
//!
//! - `dylint_toml` - set the `dylint.toml` file's contents (for testing [configurable libraries])
//! - `rustc_flags` - pass flags to the compiler when running the test
//! - `run` - run the test
//!
//! # Updating `.stderr` files
//!
//! If the standard error that results from running your `.rs` file differs from the contents of
//! your `.stderr` file, `compiletest_rs` will produce a report like the following:
//!
//! ```text
//! diff of stderr:
//!
//!  error: calling `std::env::set_var` in a test could affect the outcome of other tests
//!    --> $DIR/main.rs:8:5
//!     |
//!  LL |     std::env::set_var("KEY", "VALUE");
//!     |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//!     |
//!     = note: `-D non-thread-safe-call-in-test` implied by `-D warnings`
//!
//! -error: aborting due to previous error
//! +error: calling `std::env::set_var` in a test could affect the outcome of other tests
//! +  --> $DIR/main.rs:23:9
//! +   |
//! +LL |         std::env::set_var("KEY", "VALUE");
//! +   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//! +
//! +error: aborting due to 2 previous errors
//!
//!
//!
//! The actual stderr differed from the expected stderr.
//! Actual stderr saved to ...
//! ```
//!
//! The meaning of each line is as follows:
//!
//! - A line beginning with a plus (`+`) is in the actual standard error, but not in your `.stderr`
//!   file.
//! - A line beginning with a minus (`-`) is in your `.stderr` file, but not in the actual standard
//!   error.
//! - A line beginning with a space (` `) is in both the actual standard error and your `.stderr`
//!   file, and is provided for context.
//! - All other lines (e.g., `diff of stderr:`) contain `compiletest_rs` messages.
//!
//! **Note:** In the actual standard error, a blank line usually follows the `error: aborting due to
//! N previous errors` line. So a correct `.stderr` file will typically contain one blank line at
//! the end.
//!
//! In general, it is not too hard to update a `.stderr` file by hand. However, the `compiletest_rs`
//! report should contain a line of the form `Actual stderr saved to PATH`. Copying `PATH` to your
//! `.stderr` file should update it completely.
//!
//! Additional documentation on `compiletest_rs` can be found in [its repository].
//!
//! [Dylint]: https://github.com/trailofbits/dylint/tree/master
//! [`compiletest_rs`]: https://github.com/Manishearth/compiletest-rs
//! [`non_thread_safe_call_in_test`]: https://github.com/trailofbits/dylint/tree/master/examples/general/non_thread_safe_call_in_test/src/lib.rs
//! [`question_mark_in_expression`]: https://github.com/trailofbits/dylint/tree/master/examples/restriction/question_mark_in_expression/Cargo.toml
//! [`ui::Test::example`]: https://docs.rs/dylint_testing/latest/dylint_testing/ui/struct.Test.html#method.example
//! [`ui::Test::examples`]: https://docs.rs/dylint_testing/latest/dylint_testing/ui/struct.Test.html#method.examples
//! [`ui::Test::src_base`]: https://docs.rs/dylint_testing/latest/dylint_testing/ui/struct.Test.html#method.src_base
//! [`ui::Test`]: https://docs.rs/dylint_testing/latest/dylint_testing/ui/struct.Test.html
//! [`ui_test_example`]: https://docs.rs/dylint_testing/latest/dylint_testing/fn.ui_test_example.html
//! [`ui_test_examples`]: https://docs.rs/dylint_testing/latest/dylint_testing/fn.ui_test_examples.html
//! [`ui_test`]: https://docs.rs/dylint_testing/latest/dylint_testing/fn.ui_test.html
//! [configurable libraries]: https://github.com/trailofbits/dylint/tree/master#configurable-libraries
//! [docs.rs documentation]: https://docs.rs/dylint_testing/latest/dylint_testing/
//! [examples]: https://github.com/trailofbits/dylint/tree/master/examples
//! [its repository]: https://github.com/Manishearth/compiletest-rs

use std::{
    env::{remove_var, set_var, var_os},
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::Result;
use cargo_metadata::{Metadata, Package, Target, TargetKind};
use compiletest_rs as compiletest;
use dylint_internal::{CommandExt, env, library_filename};
use once_cell::sync::OnceCell;

mod examples;
pub mod ui;

static DRIVER: OnceCell<Result<PathBuf>> = OnceCell::new();
static LINKING_FLAGS: OnceCell<Vec<String>> = OnceCell::new();

/// Test a library on all source files in a directory.
///
/// - `name` is the name of a Dylint library to be tested. (Often, this is the same as the package
///   name.)
/// - `src_base` is a directory containing:
///   - source files on which to test the library (`.rs` files), and
///   - the output those files should produce (`.stderr` files).
pub fn ui_test(name: &str, src_base: impl AsRef<Path>) { ui::Test::src_base(name, src_base).run(); }

/// Test a library on one example target.
///
/// - `name` is the name of a Dylint library to be tested.
/// - `example` is an example target on which to test the library.
pub fn ui_test_example(name: &str, example: &str) { ui::Test::example(name, example).run(); }

/// Test a library on all example targets.
///
/// - `name` is the name of a Dylint library to be tested.
pub fn ui_test_examples(name: &str) { ui::Test::examples(name).run(); }

fn initialize(name: &str) -> &Result<PathBuf> {
    DRIVER.get_or_init(|| {
        let _ = env_logger::try_init();

        // smoelius: Try to order failures by how informative they are: failure to build the
        // library, failure to find the library, failure to build/find the driver.

        dylint_internal::cargo::build(&format!("library `{name}`"))
            .build()
            .success()?;

        // smoelius: `DYLINT_LIBRARY_PATH` must be set before `dylint_libs` is called.
        // smoelius: This was true when `dylint_libs` called `name_toolchain_map`, but that is
        // no longer the case. I am leaving the comment here for now in case removal
        // of the `name_toolchain_map` call causes a regression.
        let metadata = dylint_internal::cargo::current_metadata().unwrap();
        let dylint_library_path = metadata.target_directory.join("debug");
        unsafe {
            set_var(env::DYLINT_LIBRARY_PATH, dylint_library_path);
        }

        let dylint_libs = dylint_libs(name)?;
        let driver = dylint::driver_builder::get(
            &dylint::opts::Dylint::default(),
            env!("RUSTUP_TOOLCHAIN"),
        )?;

        unsafe {
            set_var(env::CLIPPY_DISABLE_DOCS_LINKS, "true");
            set_var(env::DYLINT_LIBS, dylint_libs);
        }

        Ok(driver)
    })
}

#[doc(hidden)]
pub fn dylint_libs(name: &str) -> Result<String> {
    let metadata = dylint_internal::cargo::current_metadata().unwrap();
    let rustup_toolchain = env::var(env::RUSTUP_TOOLCHAIN)?;
    let filename = library_filename(name, &rustup_toolchain);
    let path = metadata.target_directory.join("debug").join(filename);
    let paths = vec![path];
    serde_json::to_string(&paths).map_err(Into::into)
}

#[allow(clippy::unnecessary_wraps)]
fn example_targets(package: &Package) -> Result<Vec<Target>> {
    Ok(package
        .targets
        .iter()
        .filter(|target| target.kind == [TargetKind::Example])
        .cloned()
        .collect())
}

fn linking_flags(
    metadata: &Metadata,
    package: &Package,
    target: &Target,
) -> Result<&'static [String]> {
    LINKING_FLAGS
        .get_or_try_init(|| {
            let rustc_flags = examples::rustc_flags(metadata, package, target)?;
            let mut linking_flags = Vec::new();
            let mut iter = rustc_flags.into_iter();
            while let Some(flag) = iter.next() {
                if flag.starts_with("--edition=") {
                    linking_flags.push(flag);
                } else if flag == "--extern" || flag == "-L" {
                    let arg = examples::next(&flag, &mut iter)?;
                    linking_flags.extend([flag, arg.trim_matches('\'').to_owned()]);
                }
            }
            Ok(linking_flags)
        })
        .map(Vec::as_slice)
}

static MUTEX: Mutex<()> = Mutex::new(());

fn run_tests(driver: &Path, src_base: &Path, config: &ui::Config) {
    let _lock = MUTEX.lock().unwrap();

    // smoelius: There doesn't seem to be a way to set environment variables using `compiletest`'s
    // [`Config`](https://docs.rs/compiletest_rs/0.7.1/compiletest_rs/common/struct.Config.html)
    // struct. For comparison, where Clippy uses `compiletest`, it sets environment variables
    // directly (see: https://github.com/rust-lang/rust-clippy/blob/master/tests/compile-test.rs).
    //
    // Of course, even if `compiletest` had such support, it would need to be incorporated into
    // `dylint_testing`.

    let _var = config
        .dylint_toml
        .as_ref()
        .map(|value| VarGuard::set(env::DYLINT_TOML, value));

    let config = compiletest::Config {
        mode: compiletest::common::Mode::Ui,
        rustc_path: driver.to_path_buf(),
        src_base: src_base.to_path_buf(),
        target_rustcflags: Some(
            config.rustc_flags.clone().join(" ")
                + " --emit=metadata"
                + if cfg!(feature = "deny_warnings") {
                    " -Dwarnings"
                } else {
                    ""
                }
                + " -Zui-testing",
        ),
        ..compiletest::Config::default()
    };

    compiletest::run_tests(&config);
}

// smoelius: `VarGuard` was copied from:
// https://github.com/rust-lang/rust-clippy/blob/9cc8da222b3893bc13bc13c8827e93f8ea246854/tests/compile-test.rs
// smoelius: Clippy dropped `VarGuard` when it switched to `ui_test`:
// https://github.com/rust-lang/rust-clippy/commit/77d10ac63dae6ef0a691d9acd63d65de9b9bf88e

/// Restores an env var on drop
#[must_use]
struct VarGuard {
    key: &'static str,
    value: Option<OsString>,
}

impl VarGuard {
    fn set(key: &'static str, val: impl AsRef<OsStr>) -> Self {
        let value = var_os(key);
        unsafe {
            set_var(key, val);
        }
        Self { key, value }
    }
}

impl Drop for VarGuard {
    fn drop(&mut self) {
        match self.value.as_deref() {
            None => unsafe { remove_var(self.key) },
            Some(value) => unsafe { set_var(self.key, value) },
        }
    }
}
