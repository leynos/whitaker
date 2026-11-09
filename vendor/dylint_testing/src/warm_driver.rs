//! Prepare the pinned Dylint driver for the Makefile's warm-up step.
//!
//! `make fetch` builds the driver once, while the network is available, so the
//! test lane can reuse it offline. The preparation is fallible, so the failure
//! is reported with the context that names the step rather than as a bare
//! driver error.

use std::{io::Write, path::PathBuf};

use anyhow::{Context as _, Result};

/// Prepare the driver, write its path, and report preparation failures.
///
/// # Errors
///
/// Return an error if preparation fails, or if the path cannot be written.
///
/// # Examples
///
/// ```ignore
/// // The path comes from the crate's pinned toolchain, so it is a real build.
/// dylint_testing::warm_driver::run_from_process()?;
/// ```
pub fn run_with<F, W>(prepare: F, out: &mut W) -> Result<()>
where
    F: FnOnce() -> Result<PathBuf>,
    W: Write,
{
    let driver = prepare().context("failed to prepare Dylint driver")?;
    writeln!(out, "{}", driver.display())?;
    Ok(())
}

/// Prepare the driver with the crate's pinned toolchain.
///
/// # Errors
///
/// Return an error if preparation fails to build or locate the driver, or if
/// the path cannot be written to standard output.
pub fn run_from_process() -> Result<()> {
    run_with(crate::prepare_driver, &mut std::io::stdout().lock())
}

#[cfg(test)]
mod tests {
    //! Check that preparation failures keep the step's context.

    use std::{io::Cursor, path::PathBuf};

    use anyhow::anyhow;

    use super::run_with;

    /// Report a failed preparation with the step that failed.
    #[test]
    fn reports_a_preparation_failure_with_its_context() {
        let mut out = Cursor::new(Vec::new());

        let error = run_with(|| Err(anyhow!("driver build failed")), &mut out)
            .expect_err("a failed preparation should be reported");

        assert_eq!(
            error.to_string(),
            "failed to prepare Dylint driver",
            "the failing step should name itself"
        );
        assert_eq!(
            error.root_cause().to_string(),
            "driver build failed",
            "the underlying cause should be preserved"
        );
        assert!(out.get_ref().is_empty(), "nothing should be written");
    }

    /// Print the prepared driver path so the caller can key its cache on it.
    #[test]
    fn prints_the_prepared_driver_path() {
        let mut out = Cursor::new(Vec::new());

        run_with(|| Ok(PathBuf::from("/drivers/example")), &mut out)
            .expect("a prepared driver should be reported");

        assert_eq!(
            String::from_utf8_lossy(out.get_ref()),
            "/drivers/example\n",
            "the path should be written on its own line"
        );
    }
}
