//! Bundle header checks and clone staging for the Git adapter.
//!
//! These operations act on a bundle file rather than a repository: verifying
//! its header, listing its heads, and cloning it into a staging directory. The
//! scratch repository `git bundle verify` needs lives in
//! [`super::scratch`]; the listing parses through [`super::listing`].

use camino::Utf8Path;

use super::{ClippySnapshotError, SnapshotGit, listing::parse_bundle_heads};
use crate::clippy_snapshot::{policy::BundleHead, ports::BundleOperations};

impl BundleOperations for SnapshotGit<'_> {
    /// Check the bundle's header in a scratch repository.
    fn verify_header(&self, bundle: &Utf8Path) -> Result<(), ClippySnapshotError> {
        self.with_scratch_repository(|scratch| {
            let verify = self.output(
                "bundle verify",
                Some(scratch),
                &["bundle", "verify", bundle.as_str()],
            )?;
            if verify.status.success() {
                return Ok(());
            }
            let stderr = String::from_utf8_lossy(&verify.stderr);
            Err(ClippySnapshotError::invalid(
                bundle,
                format!("git bundle verify failed: {}", stderr.trim()),
            ))
        })
    }

    /// List the bundle's heads without touching the disk.
    fn list_heads(&self, bundle: &Utf8Path) -> Result<Vec<BundleHead>, ClippySnapshotError> {
        // Unlike `verify`, listing reads only the bundle header and works
        // outside a repository (Git 2.20 onward), so the query needs no
        // scratch state and stays a pure observation.
        let listing = self.stdout(
            "bundle list-heads",
            self.working_dir,
            &["bundle", "list-heads", bundle.as_str()],
        )?;
        parse_bundle_heads(&listing).map_err(|reason| ClippySnapshotError::invalid(bundle, reason))
    }

    /// Clone the bundle bare, then drop the remote the clone records.
    fn stage_clone(
        &self,
        bundle: &Utf8Path,
        destination: &Utf8Path,
    ) -> Result<(), ClippySnapshotError> {
        // `git bundle verify` reads only the header, so a truncated or
        // damaged pack first shows up here. Only a clone that ran and failed
        // says the bundle is invalid; a Git that could not start or timed out
        // is an infrastructure failure and keeps its own error.
        //
        // `--origin=origin` pins the remote name the clone records. Without
        // it, `clone.defaultRemoteName` renames that remote and the removal
        // below fails, stranding an otherwise valid generation.
        let clone = self.output(
            "clone",
            None,
            &[
                "clone",
                "--quiet",
                "--bare",
                "--origin=origin",
                bundle.as_str(),
                destination.as_str(),
            ],
        )?;
        if !clone.status.success() {
            let stderr = String::from_utf8_lossy(&clone.stderr);
            return Err(ClippySnapshotError::invalid(
                bundle,
                format!("the bundle could not be cloned: {}", stderr.trim()),
            ));
        }
        self.stdout(
            "remote remove",
            Some(destination),
            &["remote", "remove", "origin"],
        )?;
        Ok(())
    }
}
