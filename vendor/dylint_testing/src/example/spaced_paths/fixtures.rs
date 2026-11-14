//! Fixture inputs for the spaced dependency-path regression.
//!
//! The values are test data: the disposable workspace's files and the
//! recorder crate that captures rustc's argv. Keeping them here leaves the
//! regression logic within the repository's per-file line limit.

/// Manifest for the argv recorder, kept out of the fixture workspace.
pub(super) const RECORDER_MANIFEST: &str = r#"[package]
name = "argv_recorder"
version = "0.1.0"
edition = "2021"

[workspace]
"#;

/// A stand-in compiler that records its argv, then runs the real compiler.
pub(super) const RECORDER_SOURCE: &str = r#"//! Record the compiler arguments, then run the real compiler unchanged.

use std::{
    env,
    ffi::OsString,
    fs,
    fs::OpenOptions,
    io::Write,
    process::{Command, ExitCode},
};

/// Ends one recorded invocation.
const END: &str = "-- invocation end --";

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    log(&args);
    let compiler = env::var_os("DYLINT_TESTING_REAL_RUSTC").unwrap_or_else(|| "rustc".into());
    let status = Command::new(compiler)
        .args(&args)
        .status()
        .expect("the real compiler must start");
    ExitCode::from(status.code().unwrap_or(1) as u8)
}

/// Append the arguments rustc will execute with.
fn log(args: &[OsString]) {
    let log_path = env::var_os("DYLINT_TESTING_ARGV_LOG")
        .expect("DYLINT_TESTING_ARGV_LOG must name the log file");
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("the argv log must be writable");
    for arg in expand(args) {
        writeln!(log, "{arg}").expect("an argument must be logged");
    }
    writeln!(log, "{END}").expect("the invocation marker must be logged");
}

/// Expand `@file` response files the way rustc does: one argument per line.
///
/// The expansion is for the log only; rustc still expands the original
/// arguments itself. Fixture paths contain no line feeds, so one line per
/// argument records the boundary exactly.
fn expand(args: &[OsString]) -> Vec<String> {
    let mut expanded = Vec::new();
    for arg in args {
        match arg.to_str().and_then(|text| text.strip_prefix('@')) {
            Some(path) => {
                let contents = fs::read_to_string(path).expect("a response file must be readable");
                expanded.extend(contents.lines().map(str::to_owned));
            }
            None => expanded.push(arg.to_string_lossy().into_owned()),
        }
    }
    expanded
}
"#;

/// The fixture's files, relative to the workspace root.
///
/// The host package depends on `spaced_dep`, which re-exports a type from
/// `spaced_transitive`. Naming that re-exported type in the example forces
/// rustc to load the transitive crate's metadata, so a broken
/// `-L dependency=` value fails the compile as well as a broken `--extern`.
/// The example calls `u32::try_from`, which needs the 2021 edition prelude, so
/// a dropped `--edition=` flag also fails the compile.
pub(super) const FIXTURE_FILES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        r#"[workspace]
resolver = "2"

[package]
name = "spaced_host"
version = "0.1.0"
edition = "2021"

[dependencies]
spaced_dep = { path = "spaced dep" }
"#,
    ),
    (
        "examples/spaced_example.rs",
        r"//! Compiled by the UI harness from a workspace under a spaced path.

fn main() {
    let gadget: spaced_dep::Gadget = spaced_dep::gadget();
    let answer = u32::try_from(spaced_dep::answer()).unwrap_or_default();
    let _ = (gadget, answer);
}
",
    ),
    (
        "spaced dep/Cargo.toml",
        r#"[package]
name = "spaced_dep"
version = "0.1.0"
edition = "2021"

[dependencies]
spaced_transitive = { path = "../spaced transitive" }
"#,
    ),
    (
        "spaced dep/src/lib.rs",
        r"//! Direct dependency reached through a spaced directory path.

pub use spaced_transitive::Gadget;

/// Return a gadget from the transitive dependency.
#[must_use]
pub fn gadget() -> Gadget { Gadget::default() }

/// Return the answer from the transitive dependency.
#[must_use]
pub fn answer() -> u32 { u32::from(spaced_transitive::ANSWER) }
",
    ),
    (
        "spaced transitive/Cargo.toml",
        r#"[package]
name = "spaced_transitive"
version = "0.1.0"
edition = "2021"
"#,
    ),
    (
        "spaced transitive/src/lib.rs",
        r"//! Transitive dependency reached through a spaced directory path.

/// A value carried across the dependency boundary.
#[derive(Default)]
pub struct Gadget {
    /// The gadget's value.
    pub value: u8,
}

/// The answer used by the example.
pub const ANSWER: u8 = 42;
",
    ),
];
