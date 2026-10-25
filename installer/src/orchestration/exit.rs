//! Converts an installer operation result into the legacy process exit status.

use std::io::Write;

use crate::{error::Result, output::write_stderr_line};

/// Writes an installation error and maps the result to a process exit code.
pub fn exit_code_for_run_result(result: Result<()>, stderr: &mut dyn Write) -> i32 {
    match result {
        Ok(()) => 0,
        Err(err) => {
            write_stderr_line(stderr, err);
            1
        }
    }
}
