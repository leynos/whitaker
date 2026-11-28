//! `cargo metadata` as the source of the `check` coverage listing.
//!
//! Cargo already knows which packages a default invocation selects, and the
//! answer depends on the directory the invocation runs in. Asking Cargo keeps
//! the listing honest: `workspace_default_members` is exactly the set Cargo
//! would check, so Whitaker never reimplements the default and never drifts
//! from it.
//!
//! The query is advisory. Its failures are translated into a typed error the
//! command layer logs and skips; the lint run itself is unaffected, and Cargo
//! reports any real manifest problem through that run.

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, MetadataCommand};

use crate::ports::{
    PackageFacts,
    PackageQueryError,
    TargetFacts,
    WorkspaceFacts,
    WorkspaceQuery,
    WorkspaceQueryRequest,
};

/// Read workspace facts by running `cargo metadata`.
#[derive(Debug, Clone)]
pub struct CargoMetadataQuery {
    /// The Cargo program to run.
    ///
    /// Production names the `cargo` on `PATH`. A test names a stub so the
    /// translation can be exercised against a fixed document, and so a slow or
    /// unavailable real Cargo cannot make a unit test flaky.
    pub cargo: Utf8PathBuf,
}

impl CargoMetadataQuery {
    /// Use the `cargo` on `PATH`.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::packages::CargoMetadataQuery;
    ///
    /// let query = CargoMetadataQuery::new();
    /// assert_eq!(query.cargo, "cargo");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            cargo: Utf8PathBuf::from("cargo"),
        }
    }

    /// Run the query in `request`'s directory and read its answer.
    ///
    /// `--no-deps` keeps the query to the workspace: resolution would pull the
    /// whole dependency graph into the answer and can touch the network, while
    /// membership and defaults are already in the workspace section.
    ///
    /// # Errors
    ///
    /// Returns [`PackageQueryError`] when Cargo cannot start, rejects the
    /// workspace, or answers in a form this build cannot read.
    pub fn run(
        &self,
        request: &WorkspaceQueryRequest<'_>,
    ) -> Result<WorkspaceFacts, PackageQueryError> {
        let metadata = self.metadata(request)?;
        Ok(translate(&metadata))
    }

    /// Execute `cargo metadata` for `request`.
    fn metadata(&self, request: &WorkspaceQueryRequest<'_>) -> Result<Metadata, PackageQueryError> {
        let mut command = MetadataCommand::new();
        command.cargo_path(self.cargo.as_std_path());
        command.current_dir(request.current_dir.as_std_path());
        command.no_deps();
        if let Some(manifest) = request.manifest_path {
            command.manifest_path(manifest.as_std_path());
        }
        command.exec().map_err(classify)
    }
}

impl Default for CargoMetadataQuery {
    /// Use the `cargo` on `PATH`, as [`CargoMetadataQuery::new`] does.
    fn default() -> Self { Self::new() }
}

impl WorkspaceQuery for CargoMetadataQuery {
    /// Read the workspace facts through `cargo metadata`.
    fn workspace_facts(
        &self,
        request: &WorkspaceQueryRequest<'_>,
    ) -> Result<WorkspaceFacts, PackageQueryError> {
        self.run(request)
    }
}

/// Translate Cargo's metadata into the facts the command layer uses.
///
/// `workspace_default_members` is available from Cargo 1.71 onwards. It is
/// absent from an older Cargo, and the fallback then records no defaults, so
/// the listing shows nothing rather than showing a guess; the lint run is
/// unaffected either way.
fn translate(metadata: &Metadata) -> WorkspaceFacts {
    let by_id: BTreeMap<&str, PackageFacts> = metadata
        .packages
        .iter()
        .map(|package| (package.id.repr.as_str(), package_facts(package)))
        .collect();
    // Resolve both id lists in the order Cargo reports them. The helper that
    // filters the `packages` array would reorder the default members, and the
    // listing should follow the answer Cargo gave rather than its own order.
    let members = resolve(&metadata.workspace_members, &by_id);
    let default_members = if metadata.workspace_default_members.is_available() {
        resolve(&metadata.workspace_default_members, &by_id)
    } else {
        Vec::new()
    };
    WorkspaceFacts {
        workspace_root: metadata.workspace_root.clone(),
        members,
        default_members,
    }
}

/// Look up every package id in the order the id list gives.
fn resolve(
    ids: &[cargo_metadata::PackageId],
    by_id: &BTreeMap<&str, PackageFacts>,
) -> Vec<PackageFacts> {
    ids.iter()
        .filter_map(|id| by_id.get(id.repr.as_str()).cloned())
        .collect()
}

/// Describe one package the way the listing needs it.
fn package_facts(package: &cargo_metadata::Package) -> PackageFacts {
    PackageFacts {
        name: package.name.to_string(),
        version: package.version.to_string(),
        manifest_path: package.manifest_path.clone(),
        targets: package
            .targets
            .iter()
            .map(|target| TargetFacts {
                name: target.name.clone(),
                kinds: target.kind.iter().map(ToString::to_string).collect(),
                required_features: target.required_features.clone(),
                has_test_harness: target.test,
            })
            .collect(),
        features: package.features.clone(),
    }
}

/// Classify a `cargo metadata` failure into the port's vocabulary.
///
/// Cargo's own diagnostic is kept for the metadata case because it names the
/// manifest defect the user must fix, and it belongs to the workspace the user
/// pointed at rather than to any path Whitaker chose. A start failure is a host
/// fact, and an unreadable answer is a decode failure.
fn classify(error: cargo_metadata::Error) -> PackageQueryError {
    match error {
        cargo_metadata::Error::Io(source) => PackageQueryError::Spawn { source },
        cargo_metadata::Error::CargoMetadata { stderr } => PackageQueryError::Metadata {
            message: stderr.trim().to_owned(),
        },
        other => PackageQueryError::Decode {
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    //! The translation from Cargo's document to the port's facts.

    use std::io::Write as _;

    use camino::Utf8Path;
    use tempfile::TempDir;

    use super::{CargoMetadataQuery, translate};
    use crate::ports::{PackageQueryError, WorkspaceQuery, WorkspaceQueryRequest};

    /// A minimal `cargo metadata` document with one root package and a member.
    ///
    /// Kept as data rather than a Rust literal so the document can be read and
    /// compared as JSON.
    const DOCUMENT: &str = include_str!("packages_metadata_fixture.json");

    /// Parse the fixture document.
    fn metadata() -> cargo_metadata::Metadata {
        serde_json::from_str(DOCUMENT).expect("the fixture document must parse")
    }

    /// Keep membership, defaults and the root, in Cargo's order.
    #[test]
    fn the_translation_keeps_cargos_own_answer() {
        let facts = translate(&metadata());

        assert_eq!(facts.workspace_root.as_str(), "/work/app");
        assert_eq!(
            facts
                .members
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            ["rootpkg", "member_a"],
            "every workspace member must be listed"
        );
        assert_eq!(
            facts
                .default_members
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            ["member_a"],
            "the default selection must be Cargo's, not a guess"
        );
        let member = facts
            .members
            .iter()
            .find(|package| package.name == "member_a")
            .expect("member_a should be a member");
        assert!(
            member.has_library(),
            "the library target must survive the translation"
        );
        assert!(
            member
                .targets
                .iter()
                .any(|target| target.is_binary() && target.name == "tool"),
            "the binary target and its name must survive the translation"
        );
    }

    /// Write a `cargo` stub that prints `body` on stdout, `message` on
    /// stderr, and exits with `status`.
    ///
    /// The stub is a POSIX shell script, so it only exists on Unix. The write
    /// goes through `cap_std`, which is the repository's policy for filesystem
    /// access: a test helper is still code the lint suite compiles.
    #[cfg(unix)]
    fn stub_cargo(dir: &TempDir, body: &str, message: &str, status: i32) -> CargoMetadataQuery {
        use cap_std::{
            ambient_authority,
            fs::{Dir, Permissions},
        };

        let fs = Dir::open_ambient_dir(dir.path(), ambient_authority())
            .expect("the stub directory must open");
        let mut file = fs.create("cargo").expect("the stub must be created");
        writeln!(
            file,
            "#!/bin/sh\nprintf '%s\\n' \"{message}\" >&2\ncat <<'JSON'\n{body}\nJSON\nexit \
             {status}"
        )
        .expect("the stub must be written");
        drop(file);
        {
            use cap_std::fs::PermissionsExt as _;
            fs.set_permissions("cargo", Permissions::from_mode(0o755))
                .expect("the stub must be executable");
        }
        CargoMetadataQuery {
            cargo: Utf8Path::from_path(&dir.path().join("cargo"))
                .expect("the stub path is UTF-8")
                .to_path_buf(),
        }
    }

    /// Read a workspace through a stub Cargo.
    #[cfg(unix)]
    #[test]
    fn a_real_query_reads_the_stub_answer() {
        let dir = TempDir::new().expect("a temporary directory is available");
        let query = stub_cargo(&dir, DOCUMENT, "", 0);

        let facts = query
            .workspace_facts(&WorkspaceQueryRequest {
                current_dir: Utf8Path::from_path(dir.path()).expect("the path is UTF-8"),
                manifest_path: None,
            })
            .expect("the stub answer parses");

        assert_eq!(
            facts.members.len(),
            2,
            "the adapter must report the workspace it read"
        );
    }

    /// Keep Cargo's own diagnostic when it rejects the workspace.
    #[cfg(unix)]
    #[test]
    fn a_rejected_workspace_keeps_cargos_diagnostic() {
        let dir = TempDir::new().expect("a temporary directory is available");
        let query = stub_cargo(&dir, "", "  the workspace is not a package  ", 101);

        let error = query
            .workspace_facts(&WorkspaceQueryRequest {
                current_dir: Utf8Path::from_path(dir.path()).expect("the path is UTF-8"),
                manifest_path: None,
            })
            .expect_err("a rejected workspace must be reported");

        let PackageQueryError::Metadata { message } = error else {
            panic!("an exit status from Cargo is a metadata failure, got {error:?}");
        };
        assert_eq!(
            message, "the workspace is not a package",
            "Cargo's diagnostic must be kept and trimmed, not discarded"
        );
    }

    /// Treat an unstartable Cargo as a spawn failure.
    #[test]
    fn a_missing_cargo_is_a_spawn_failure() {
        let dir = TempDir::new().expect("a temporary directory is available");
        let query = CargoMetadataQuery {
            cargo: "/nonexistent/cargo".into(),
        };

        let error = query
            .workspace_facts(&WorkspaceQueryRequest {
                current_dir: Utf8Path::from_path(dir.path()).expect("the path is UTF-8"),
                manifest_path: None,
            })
            .expect_err("a missing Cargo must be reported");

        assert!(
            matches!(error, PackageQueryError::Spawn { .. }),
            "an unstartable Cargo is a spawn failure, got {error:?}"
        );
    }
}
