//! Artefact naming policy for prebuilt lint library archives.
//!
//! Constructs deterministic archive names in the format specified by ADR-001:
//! `whitaker-lints-<git_sha>-<toolchain>-<target>.tar.zst`.

use super::git_sha::GitSha;
use super::target::TargetTriple;
use super::toolchain_channel::ToolchainChannel;
use std::fmt;

/// The fixed prefix for all artefact archive names.
const ARTEFACT_PREFIX: &str = "whitaker-lints";

/// The fixed file extension for artefact archives.
const ARTEFACT_EXTENSION: &str = ".tar.zst";

/// A fully-qualified artefact archive name.
///
/// Constructed from a git SHA, toolchain channel, and target triple, this
/// type produces the deterministic filename defined in ADR-001.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::naming::ArtefactName;
/// use whitaker_installer::artefact::git_sha::GitSha;
/// use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;
/// use whitaker_installer::artefact::target::TargetTriple;
///
/// let sha: GitSha = "abc1234".try_into().expect("valid git SHA");
/// let toolchain: ToolchainChannel = "nightly-2025-09-18"
///     .try_into()
///     .expect("valid toolchain channel");
/// let target: TargetTriple = "x86_64-unknown-linux-gnu"
///     .try_into()
///     .expect("valid target triple");
///
/// let name = ArtefactName::new(sha, toolchain, target);
/// assert_eq!(
///     name.to_string(),
///     "whitaker-lints-abc1234-nightly-2025-09-18-x86_64-unknown-linux-gnu.tar.zst"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtefactName {
    git_sha: GitSha,
    toolchain: ToolchainChannel,
    target: TargetTriple,
}

impl ArtefactName {
    /// Create an artefact name from validated components.
    #[must_use]
    pub fn new(git_sha: GitSha, toolchain: ToolchainChannel, target: TargetTriple) -> Self {
        Self {
            git_sha,
            toolchain,
            target,
        }
    }

    /// Return the git SHA component.
    #[must_use]
    pub fn git_sha(&self) -> &GitSha {
        &self.git_sha
    }

    /// Return the toolchain channel component.
    #[must_use]
    pub fn toolchain(&self) -> &ToolchainChannel {
        &self.toolchain
    }

    /// Return the target triple component.
    #[must_use]
    pub fn target(&self) -> &TargetTriple {
        &self.target
    }

    /// Return the filename as a string without consuming the value.
    #[must_use]
    pub fn filename(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for ArtefactName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{ARTEFACT_PREFIX}-{}-{}-{}{}",
            self.git_sha, self.toolchain, self.target, ARTEFACT_EXTENSION
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn sample_name() -> ArtefactName {
        ArtefactName::new(
            GitSha::try_from("abc1234").expect("valid sha"),
            ToolchainChannel::try_from("nightly-2025-09-18").expect("valid channel"),
            TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"),
        )
    }

    #[rstest]
    fn display_matches_adr_format(sample_name: ArtefactName) {
        assert_eq!(
            sample_name.to_string(),
            concat!(
                "whitaker-lints-abc1234-nightly-2025-09-18",
                "-x86_64-unknown-linux-gnu.tar.zst"
            )
        );
    }

    #[rstest]
    fn filename_matches_display(sample_name: ArtefactName) {
        assert_eq!(sample_name.filename(), sample_name.to_string());
    }

    #[rstest]
    fn accessors_return_components(sample_name: ArtefactName) {
        assert_eq!(sample_name.git_sha().as_str(), "abc1234");
        assert_eq!(sample_name.toolchain().as_str(), "nightly-2025-09-18");
        assert_eq!(sample_name.target().as_str(), "x86_64-unknown-linux-gnu");
    }

    #[rstest]
    fn different_targets_produce_different_names() {
        let sha = GitSha::try_from("abc1234").expect("valid");
        let ch = ToolchainChannel::try_from("nightly-2025-09-18").expect("valid");

        let linux = ArtefactName::new(
            sha.clone(),
            ch.clone(),
            TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid"),
        );
        let macos = ArtefactName::new(
            sha,
            ch,
            TargetTriple::try_from("aarch64-apple-darwin").expect("valid"),
        );

        assert_ne!(linux.to_string(), macos.to_string());
    }
}
