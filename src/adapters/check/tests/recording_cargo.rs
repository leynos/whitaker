//! A `cargo` stub that records every start, so a refused run is provable.
//!
//! The record has to be written by the child: a flag in the test process would
//! be set whether or not the command ran. The stub is Unix-only for the same
//! reason the readiness stub is a shell script, and the platform-neutral
//! refusal is pinned by a test that inspects the returned error.

use std::{ffi::OsStr, path::PathBuf};

use tempfile::TempDir;

/// A `cargo` stub that appends a line to its log every time it starts.
///
/// A POSIX shell script is enough for the same reason the readiness stub is
/// one. The platform-neutral refusal is pinned by
/// `a_user_path_that_changed_before_the_run_is_refused`.
pub(super) struct RecordingCargo {
    /// Owns the directory the stub lives in.
    _directory: TempDir,
    /// The program the run is asked to start.
    program: PathBuf,
    /// The file the stub writes when it starts.
    log: PathBuf,
}

impl RecordingCargo {
    /// Write a stub that appends a line to its log and exits successfully.
    pub(super) fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = TempDir::new().expect("a temporary directory is available");
        let log = directory.path().join("stub.log");
        let program = directory.path().join("cargo");
        // The log is named relative to the script, so the generated shell has
        // no interpolated path to quote: a `TMPDIR` containing an apostrophe
        // would otherwise end the literal early and the stub would fail before
        // it recorded a launch.
        let script = concat!(
            "#!/bin/sh\n",
            "printf 'started\\n' >> \"$(dirname \"$0\")/stub.log\"\n",
            "exit 0\n",
        );
        std::fs::write(&program, script).expect("the stub cargo can be written");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("the stub cargo can be made executable");
        Self {
            _directory: directory,
            program,
            log,
        }
    }

    /// The program the run is asked to start.
    pub(super) fn program(&self) -> &OsStr { self.program.as_os_str() }

    /// How many times the stub was started.
    pub(super) fn starts(&self) -> usize {
        std::fs::read_to_string(&self.log).map_or(0, |log| log.lines().count())
    }
}
