//! The workspace-metadata port behind `whitaker check -v`.
//!
//! `check` shows the packages Cargo will check, and the only trustworthy
//! source for that answer is Cargo itself. The adapter asks `cargo metadata`
//! and translates the answer into the plain facts here; the command layer
//! decides which facts to display and never learns a `cargo_metadata` type.
//!
//! The default selection is not reimplemented. Cargo reports
//! `workspace_default_members` for the context the query ran in — the current
//! package, the configured `default-members`, or every member of a virtual
//! workspace — so a divergence between Whitaker's idea of "the default" and
//! Cargo's is impossible by construction.

use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use thiserror::Error;

/// One build target of a package, as Cargo describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFacts {
    /// The target name; for a binary, the name `--bin` selects.
    pub name: String,
    /// Cargo's target kinds, such as `lib`, `bin`, `test`, `example` or `bench`.
    pub kinds: Vec<String>,
}

impl TargetFacts {
    /// Report whether the target is a library.
    ///
    /// Cargo reports a library's kind as its crate type: `lib` (and
    /// `proc-macro`) for the default, and `rlib`, `dylib`, `cdylib` or
    /// `staticlib` when the manifest selects one. All of them are the package's
    /// library target, which `--lib` selects.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::TargetFacts;
    ///
    /// let target = TargetFacts {
    ///     name: "app".to_owned(),
    ///     kinds: vec!["bin".to_owned()],
    /// };
    /// assert!(target.is_binary());
    /// assert!(!target.is_library());
    /// ```
    #[must_use]
    pub fn is_library(&self) -> bool {
        self.kinds.iter().any(|kind| {
            matches!(
                kind.as_str(),
                "lib" | "proc-macro" | "rlib" | "dylib" | "cdylib" | "staticlib"
            )
        })
    }

    /// Report whether the target is a binary.
    #[must_use]
    pub fn is_binary(&self) -> bool { self.kinds.iter().any(|kind| kind == "bin") }

    /// Report whether the target is an integration test.
    #[must_use]
    pub fn is_test(&self) -> bool { self.kinds.iter().any(|kind| kind == "test") }

    /// Report whether the target is an example.
    #[must_use]
    pub fn is_example(&self) -> bool { self.kinds.iter().any(|kind| kind == "example") }

    /// Report whether the target is a benchmark.
    #[must_use]
    pub fn is_bench(&self) -> bool { self.kinds.iter().any(|kind| kind == "bench") }
}

/// One workspace package, as Cargo describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFacts {
    /// The package name.
    pub name: String,
    /// The package version.
    pub version: String,
    /// The path to the package's `Cargo.toml`.
    pub manifest_path: Utf8PathBuf,
    /// The package's build targets.
    ///
    /// The coverage listing uses them to apply the target selectors before it
    /// claims a package will be checked. An empty list means the facts carry no
    /// target information, and the listing then keeps the package rather than
    /// hiding coverage it cannot rule out.
    pub targets: Vec<TargetFacts>,
}

impl PackageFacts {
    /// The directory holding the package's manifest.
    ///
    /// Cargo's own `Checking` line names the package directory, so a caller
    /// that renders that line needs the parent rather than the manifest.
    /// A manifest at a filesystem root has no parent, and the manifest path
    /// itself is then the most useful answer.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::PackageFacts;
    ///
    /// let package = PackageFacts {
    ///     name: "app".to_owned(),
    ///     version: "0.1.0".to_owned(),
    ///     manifest_path: "/work/app/Cargo.toml".into(),
    ///     targets: Vec::new(),
    /// };
    /// assert_eq!(package.directory().as_str(), "/work/app");
    /// ```
    #[must_use]
    pub fn directory(&self) -> &Utf8Path {
        self.manifest_path
            .parent()
            .unwrap_or(self.manifest_path.as_path())
    }

    /// Report whether the package has a library target.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::{PackageFacts, TargetFacts};
    ///
    /// let package = PackageFacts {
    ///     name: "app".to_owned(),
    ///     version: "0.1.0".to_owned(),
    ///     manifest_path: "/work/app/Cargo.toml".into(),
    ///     targets: vec![TargetFacts {
    ///         name: "app".to_owned(),
    ///         kinds: vec!["lib".to_owned()],
    ///     }],
    /// };
    /// assert!(package.has_library());
    /// ```
    #[must_use]
    pub fn has_library(&self) -> bool { self.targets.iter().any(TargetFacts::is_library) }

    /// Report whether the package has a binary target.
    #[must_use]
    pub fn has_binary(&self) -> bool { self.targets.iter().any(TargetFacts::is_binary) }

    /// Report whether the package has an integration-test target.
    #[must_use]
    pub fn has_test(&self) -> bool { self.targets.iter().any(TargetFacts::is_test) }

    /// Report whether the package has an example target.
    #[must_use]
    pub fn has_example(&self) -> bool { self.targets.iter().any(TargetFacts::is_example) }

    /// Report whether the package has a benchmark target.
    #[must_use]
    pub fn has_bench(&self) -> bool { self.targets.iter().any(TargetFacts::is_bench) }
}

/// A package selector, as Cargo accepts it.
///
/// The value is deliberately opaque. Whitaker forwards it unchanged and only
/// interprets enough of its shape — a name, a `name@version` pair, a glob
/// pattern, or a package-ID source — to render the coverage listing. Cargo
/// remains the authority on what a selector matches.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageSpec(String);

impl PackageSpec {
    /// Wrap a selector exactly as the caller wrote it.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::PackageSpec;
    ///
    /// let spec = PackageSpec::new("member_*");
    /// assert_eq!(spec.as_str(), "member_*");
    /// ```
    #[must_use]
    pub fn new(spec: impl Into<String>) -> Self { Self(spec.into()) }

    /// Borrow the selector text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

impl From<&str> for PackageSpec {
    /// Wrap a borrowed selector.
    fn from(spec: &str) -> Self { Self(spec.to_owned()) }
}

impl From<String> for PackageSpec {
    /// Wrap an owned selector.
    fn from(spec: String) -> Self { Self(spec) }
}

impl AsRef<str> for PackageSpec {
    /// Borrow the selector text.
    fn as_ref(&self) -> &str { &self.0 }
}

impl std::fmt::Display for PackageSpec {
    /// Render the selector exactly as the caller wrote it.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The workspace facts a selection query needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceFacts {
    /// The workspace root directory.
    pub workspace_root: Utf8PathBuf,
    /// Every workspace member, in Cargo's order.
    pub members: Vec<PackageFacts>,
    /// Cargo's answer to "what would a default invocation select".
    ///
    /// This is context-dependent: at the workspace root it is the configured
    /// `default-members` (or the root package), inside a member it is that
    /// member, and in a virtual workspace it is every member. The adapter reads
    /// it from `cargo metadata` rather than deriving it.
    pub default_members: Vec<PackageFacts>,
}

/// What the workspace query needs from the caller.
#[derive(Debug, Clone, Copy)]
pub struct WorkspaceQueryRequest<'a> {
    /// The directory the query runs in.
    ///
    /// It decides which package Cargo treats as current, so it must be the
    /// directory the lint run itself will use.
    pub current_dir: &'a Utf8Path,
    /// The manifest Cargo should read instead of searching `current_dir`.
    pub manifest_path: Option<&'a Utf8Path>,
}

/// Why the workspace facts could not be read.
#[derive(Debug, Error)]
pub enum PackageQueryError {
    /// Cargo could not be started.
    #[error("could not start cargo metadata: {source}")]
    Spawn {
        /// The operating-system process error.
        #[source]
        source: io::Error,
    },
    /// Cargo ran but rejected the workspace.
    #[error("cargo metadata failed: {message}")]
    Metadata {
        /// Cargo's own diagnostic, already trimmed.
        message: String,
    },
    /// Cargo answered with something this build cannot interpret.
    #[error("could not interpret cargo metadata: {message}")]
    Decode {
        /// A description of the unexpected answer.
        message: String,
    },
}

/// Reads the workspace facts Cargo itself would use.
pub trait WorkspaceQuery {
    /// Read the workspace, default members and package list.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use camino::Utf8Path;
    /// use whitaker::{
    ///     adapters::SystemServices,
    ///     ports::{WorkspaceQuery, WorkspaceQueryRequest},
    /// };
    ///
    /// let services = SystemServices::new();
    /// let facts = services.workspace_facts(&WorkspaceQueryRequest {
    ///     current_dir: Utf8Path::new("."),
    ///     manifest_path: None,
    /// })?;
    /// println!("{} packages", facts.members.len());
    /// # Ok::<(), whitaker::ports::PackageQueryError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`PackageQueryError`] when Cargo cannot be started, rejects the
    /// workspace, or answers in a form this build cannot read.
    fn workspace_facts(
        &self,
        request: &WorkspaceQueryRequest<'_>,
    ) -> Result<WorkspaceFacts, PackageQueryError>;
}
