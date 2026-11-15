//! Manifest-scanning toolkit for the architecture-fitness guard.
//!
//! `architecture_boundary.rs` states ADR 0013's dependency rule and asserts it
//! over the real manifests. This module supplies the machinery those assertions
//! are built from: it parses a manifest, walks every dependency table it
//! declares, and decides whether one entry names a given package.
//!
//! A dependency's effective name is not always written in the manifest that
//! declares it. A member may rename an edge locally (`loc = { package = … }`)
//! or inherit an entire identity from the workspace (`loc = { workspace =
//! true }`), and both shapes are resolved, in Cargo's order: inheritance first,
//! then the local entry. The inherited shape needs the root
//! `[workspace.dependencies]` table, which is read alongside every real
//! manifest.
//!
//! **The scan fails closed.** An entry whose name cannot be read is reported as
//! `Unresolved`, not as `Absent`, and the guards treat only `Absent` as a pass.
//! An entry that inherits a workspace declaration the scan cannot reach might be
//! the forbidden package under an innocuous key, so reading it as absent would
//! be a false clean bill of health.
//!
//! **Discovery fails closed the same way.** Finding a manifest has three
//! outcomes — present, confirmed absent, undecidable — and the third is carried
//! as an error rather than folded into the second. A permission denial reads as
//! `false` from `is_file()` exactly as a missing file does, so an `Option`-only
//! answer would let a guard that could not look report the same clean result as
//! one that looked and found nothing.
//!
//! Precedent: `crates/whitaker_clones_core/build_support.rs` parses a manifest
//! with `toml::Table` and walks its dependency tables, and separates a missing
//! file from an unreadable one in the same three-way form.

/// Behavioural tests for which tables the scan treats as dependency tables.
///
/// A `mod` inside a test binary compiles unconditionally, so these pull the
/// tests in whenever the toolkit is used — which is what makes them run.
mod table_discovery;

/// Behavioural tests for the workspace-inheritance resolution path.
mod workspace_inheritance;

/// Why discovery could not decide whether a manifest is present.
mod discovery_error;

/// The composed guard path that both real and fixture manifests travel.
mod guard;

/// Behavioural tests of the guard path, including the cases it must reject.
mod guard_tests;

/// Behavioural tests for the guard's manifest-load failure branch.
mod guard_load_tests;

/// Unit tests for manifest discovery's error and absence reporting.
mod discovery_tests;

/// Behavioural tests for loading a located manifest, and for discovery's
/// propagation of a load failure.
mod manifest_loading_tests;

/// Property tests for the scan's Found/Unresolved precedence.
mod property;

/// Walking the dependency tables Cargo actually reads.
mod table_walk;

/// Finding a crate's manifest on disk.
mod path_discovery;

/// Reading a manifest from disk, with every failure carried as a typed error.
mod manifest_io;

/// A staged temporary directory for tests that must touch the filesystem.
mod scratch;

pub(crate) use discovery_error::DiscoveryError;
pub(crate) use guard::{assert_no_edge, assert_no_edge_in};
pub(crate) use manifest_io::load_manifest;
pub(crate) use path_discovery::{
    declared_path_in,
    manifest_if_present,
    probe,
    resolve_declared,
    workspace_dependencies,
    workspace_dependencies_in,
};
pub(crate) use table_walk::dependency_tables;

/// The dependency tables a guard inspects.
///
/// `[dependencies]` is the production edge ADR 0013 governs. The dev and build
/// tables are inspected too, because a cycle is a cycle whichever table carries
/// it, and because a dev-dependency on `whitaker-common` would put the
/// localization stack into the dependent crate's test build.
pub(crate) const DEPENDENCY_TABLES: [&str; 3] =
    ["dependencies", "dev-dependencies", "build-dependencies"];

/// A dependency table's path within the manifest, and its entries.
pub(crate) type DependencyTable = (String, toml::Table);

/// The outcome of scanning a manifest for one package name.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScanOutcome {
    /// The package was declared; the payload is the table and key that named it.
    Found(String),
    /// The package was not declared.
    Absent,
    /// An entry inherits from the workspace, but the table it inherits from
    /// does not carry a readable declaration for it, so the entry's effective
    /// name cannot be determined. The payload is the table and key of the
    /// inheriting entry.
    Unresolved(String),
}

impl ScanOutcome {
    /// Returns whether the package was declared.
    #[must_use]
    pub(crate) const fn is_found(&self) -> bool { matches!(self, Self::Found(_)) }

    /// Returns whether the scan certified that the package is absent.
    ///
    /// An unresolved entry is not a certification. Its unknown name may be the
    /// package under scan, so the scan fails closed rather than reporting a
    /// manifest it could only partly read.
    #[must_use]
    pub(crate) const fn is_absent(&self) -> bool { matches!(self, Self::Absent) }

    /// Describes what the scan found, for an assertion message.
    #[must_use]
    pub(crate) fn finding(&self) -> String {
        match self {
            Self::Found(where_) => format!("declares it at {where_}"),
            Self::Unresolved(where_) => format!(
                "leaves {where_} unresolved, inheriting a workspace declaration this scan could \
                 not read"
            ),
            Self::Absent => "declares no such dependency, contradicting this assertion".to_owned(),
        }
    }
}

/// Returns whether a dependency entry's effective name is `package`.
///
/// An entry names its package in one of two shapes, and the order they are
/// tried in is Cargo's own resolution order, not a preference:
///
/// 1. **Inheritance.** `loc = { workspace = true }` takes its whole identity from the root
///    `[workspace.dependencies]` entry for `loc`. This is the one shape a member manifest cannot be
///    read in isolation to resolve.
/// 2. **The local entry.** `loc = { package = "serde" }` depends on `serde`, and `loc = "1"`
///    depends on `loc`.
///
/// Inheritance is tried first because Cargo resolves it first, and the
/// difference is observable in both directions. A member may inherit a rename
/// it cannot see — `loc = { workspace = true }` resolving to `serde` — and a
/// member that *does* write a local `package` beside `workspace = true` is
/// writing a key Cargo discards with `unused manifest key: …package`; the
/// inherited identity wins outright.
///
/// `workspace_dependencies` is the root workspace's `[workspace.dependencies]`
/// table, or `None` when the manifest under scan *is* that root, which has no
/// parent to inherit from. An inheritance with no readable declaration behind
/// it reports `Unresolved` rather than `Absent`, so the guard fails closed: an
/// entry whose name cannot be read is not evidence that the package is missing.
fn names_package(
    key: &str,
    value: &toml::Value,
    package: &str,
    workspace_dependencies: Option<&toml::Table>,
) -> EntryVerdict {
    let inherits = value
        .get("workspace")
        .and_then(toml::Value::as_bool)
        .is_some_and(|flag| flag);
    if inherits {
        let Some(declaration) = workspace_dependencies.and_then(|table| table.get(key)) else {
            return EntryVerdict::Unresolved;
        };
        return if declaration
            .get("package")
            .and_then(toml::Value::as_str)
            .unwrap_or(key)
            == package
        {
            EntryVerdict::Names
        } else {
            EntryVerdict::Other
        };
    }

    // The key is the *local* name, not the package: Cargo resolves an entry by
    // its `package` override when one is written and by the key only when none
    // is. So `whitaker-common = { package = "serde" }` depends on `serde`, and
    // reading the key as the identity would report a forbidden edge that is not
    // there. The override is consulted first for that reason.
    let named_locally = value.get("package").and_then(toml::Value::as_str);
    if named_locally.unwrap_or(key) == package {
        return EntryVerdict::Names;
    }

    EntryVerdict::Other
}

/// What reading one dependency entry established about the package under scan.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EntryVerdict {
    /// The entry's effective name is the package under scan.
    Names,
    /// The entry's effective name is readable, and is some other package.
    Other,
    /// The entry inherits from the workspace, which does not declare the key,
    /// so the entry's effective name cannot be read at all.
    Unresolved,
}

/// Asserts that scanning a manifest examined at least one dependency entry.
///
/// This is the non-vacuity floor. A path typo, a renamed table, or a
/// restructure must not be able to make a guard pass by examining nothing.
///
/// # Panics
///
/// Panics when the manifest declares no dependency table, or no entry within
/// one, which would make the calling guard vacuous.
pub(crate) fn assert_non_vacuous(document: &toml::Table, crate_name: &str) {
    let tables = dependency_tables(document);
    let entries: usize = tables.iter().map(|(_, table)| table.len()).sum();
    assert!(
        !tables.is_empty(),
        "{crate_name}/Cargo.toml declares no dependency table; the guard is vacuous"
    );
    assert!(
        entries > 0,
        "{crate_name}/Cargo.toml declares no dependency; the guard is vacuous"
    );
}

/// Scans a manifest for a dependency, naming where it was found.
///
/// A declaration outranks an unresolved entry: an explicit match certifies the
/// package's presence even when some other entry could not be read. Only when no
/// entry names the package does an unresolved entry decide the outcome, and then
/// it is reported as `Unresolved` rather than `Absent`.
///
/// A declaration anywhere outranks an unreadable entry, and the scan is
/// order-independent: it visits every entry, so an `Unresolved` can never
/// short-circuit past a later `Found`. `toml::Table` happens to iterate in key
/// order, but the verdict must not rest on that — an unreadable key that sorts
/// ahead of a declaring key would otherwise decide the outcome.
///
/// `workspace_dependencies` is the root workspace's `[workspace.dependencies]`
/// table, if the manifest under scan inherits from one. Fixture manifests pass
/// `None`, which leaves any `{ workspace = true }` entry they contain
/// unresolved: they exercise the local shapes, and the inherited shape is
/// covered separately by `inherited_rename_is_resolved_through_the_workspace`.
pub(crate) fn scan_for(
    document: &toml::Table,
    package: &str,
    workspace_dependencies: Option<&toml::Table>,
) -> ScanOutcome {
    let mut unresolved = None;
    for (table, entries) in dependency_tables(document) {
        for (key, value) in &entries {
            let where_ = format!("{table}.{key}");
            match names_package(key, value, package, workspace_dependencies) {
                EntryVerdict::Names => return ScanOutcome::Found(where_),
                EntryVerdict::Unresolved => unresolved = Some(where_),
                EntryVerdict::Other => {}
            }
        }
    }
    unresolved.map_or(ScanOutcome::Absent, ScanOutcome::Unresolved)
}

/// Parses a manifest **fixture literal** into a TOML table.
///
/// This panicking parser is for fixture literals only — strings in this crate's
/// own source — and every call site is one. A literal that does not parse is a
/// typo in the test, which is a defect rather than a condition under test, and
/// the callers include `#[should_panic]` cases that the workspace's
/// `panic_in_result_fn` forbids from returning `Result`.
///
/// A manifest read from **disk** goes through [`load_manifest`], which returns
/// a [`DiscoveryError`] instead: the file can be missing, refused, or written by
/// a different branch, and none of that is a defect in the test.
///
/// # Panics
///
/// Panics when the literal is not valid TOML, naming the parse failure.
pub(crate) fn parse_manifest(manifest: &str) -> toml::Table {
    match manifest.parse::<toml::Table>() {
        Ok(parsed) => parsed,
        Err(error) => panic!("manifest fixture should be valid TOML: {error}"),
    }
}

/// Returns the root workspace's `[workspace.dependencies]` table, given the
/// root manifest that declares it.
///
/// This is the resolution step behind `workspace_dependencies`, split out so
/// fixture tests can build a table from a literal instead of a file. A fixture
/// that inherits through `{ workspace = true }` cannot be resolved without one.
///
/// Returns `None` when the document carries no such table, which leaves every
/// inheriting entry unresolved — the fail-closed reading.
#[must_use]
pub(crate) fn workspace_table_of(document: &toml::Table) -> Option<toml::Table> {
    document
        .get("workspace")?
        .get("dependencies")?
        .as_table()
        .cloned()
}
