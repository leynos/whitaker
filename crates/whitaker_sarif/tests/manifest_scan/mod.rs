//! Manifest-scanning toolkit for the architecture-fitness guard.
//!
//! `architecture_boundary.rs` states ADR 005's dependency rule and asserts it
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
//! Precedent: `crates/whitaker_clones_core/build_support.rs` parses a manifest
//! with `toml::Table` and walks its dependency tables.

/// Behavioural tests for which tables the scan treats as dependency tables.
///
/// A `mod` inside a test binary compiles unconditionally, so these pull the
/// tests in whenever the toolkit is used — which is what makes them run.
mod table_discovery;

/// Behavioural tests for the workspace-inheritance resolution path.
mod workspace_inheritance;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The dependency tables a guard inspects.
///
/// `[dependencies]` is the production edge ADR 005 governs. The dev and build
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

/// Joins a table path prefix to a key, omitting an empty prefix.
pub(crate) fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Harvests the dependency tables `scope` may declare directly.
///
/// Every name in `DEPENDENCY_TABLES` is harvested under `prefix`, and nothing
/// deeper: a dependency table is read only where Cargo reads it, which is
/// always as a direct child of the scope this is called on.
fn harvest_under(scope: &toml::Table, prefix: &str, into: &mut Vec<DependencyTable>) {
    for name in DEPENDENCY_TABLES {
        if let Some(table) = scope.get(name).and_then(toml::Value::as_table) {
            into.push((join_path(prefix, name), table.clone()));
        }
    }
}

/// Collects every dependency table a manifest declares.
///
/// Cargo reads dependencies in exactly three places, and this visits each by
/// name rather than by recursing on the key `dependencies`:
///
/// 1. the manifest's own top level, including `dev-` and `build-dependencies`;
/// 2. a root manifest's `[workspace.dependencies]`, which every member inheriting `{ workspace =
///    true }` resolves against;
/// 3. the body of a `[target.<selector>]` table.
///
/// A `dependencies` key anywhere else is not an edge. Cargo never resolves
/// `[package.metadata.tool.dependencies]`, and it *silently ignores* any table
/// below a target selector: neither
/// `[target.'cfg(unix)'.metadata.dependencies]` nor `[target.a.b.dependencies]`
/// resolves anything. A walk matching on the table name alone, or descending on
/// a `target.` prefix, would report edges Cargo does not have. Visiting the
/// three places explicitly keeps the scan on Cargo's grammar by construction.
pub(crate) fn dependency_tables(document: &toml::Table) -> Vec<DependencyTable> {
    let mut tables = Vec::new();

    harvest_under(document, "", &mut tables);

    // Named on its own rather than harvested beside the other workspace tables:
    // Cargo accepts `[workspace.dev-dependencies]` and
    // `[workspace.build-dependencies]`, silently ignores both, and rejects a
    // member that inherits from one. `workspace.dependencies` is the whole of
    // the workspace's contribution, so it is the only one read.
    if let Some(table) = document
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        tables.push(("workspace.dependencies".to_owned(), table.clone()));
    }

    // Exactly one selector level, never more: the dependency tables are
    // `target`'s grandchildren, and a `[target.<selector>]` body is also where
    // the `dev-` and `build-` forms may appear.
    if let Some(targets) = document.get("target").and_then(toml::Value::as_table) {
        for (selector, value) in targets {
            if let Some(body) = value.as_table() {
                harvest_under(body, &join_path("target", selector), &mut tables);
            }
        }
    }

    tables
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

/// Returns the root workspace's `[workspace.dependencies]` table.
///
/// A member manifest may inherit a dependency — and its rename — through
/// `{ workspace = true }`, in which case the package name lives only in the
/// root manifest. Resolving that is what stops an inherited rename from being
/// an invisible edge.
///
/// Returns `None` when no root is reachable, which is the correct answer for a
/// fixture and a fail-closed one for a real manifest: an inherited entry whose
/// declaration cannot be read is not treated as clean.
///
/// # Panics
///
/// Panics when a root manifest is found but cannot be parsed, which means the
/// workspace this guard navigates has changed shape.
pub(crate) fn workspace_dependencies() -> Option<toml::Table> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent()?.parent()?.join("Cargo.toml");
    if !root.is_file() {
        return None;
    }
    let document = parse_manifest(&read_manifest(&root));
    workspace_table_of(&document)
}

/// Parses a manifest into a TOML table.
///
/// # Panics
///
/// Panics when the manifest is not valid TOML. Every input is either a tracked
/// file or a fixture literal, so a parse failure is a defect in this test rather
/// than a condition under test.
pub(crate) fn parse_manifest(manifest: &str) -> toml::Table {
    match manifest.parse::<toml::Table>() {
        Ok(parsed) => parsed,
        Err(error) => panic!("manifest should be valid TOML: {error}"),
    }
}

/// Reads a manifest through a capability-scoped `cap_std` directory handle.
///
/// Filesystem access is capability-scoped rather than ambient, per the
/// `no_std_fs_operations` lint; the handle is opened over the manifest's own
/// parent directory, so the capability granted is no wider than the file read.
///
/// # Panics
///
/// Panics when the manifest cannot be read, which means the layout this guard
/// navigates has changed.
pub(crate) fn read_manifest(path: &Utf8Path) -> String {
    let Some(directory) = path.parent() else {
        panic!("manifest path {path} has no parent directory");
    };
    let Some(filename) = path.file_name() else {
        panic!("manifest path {path} has no file name");
    };
    let handle = match Dir::open_ambient_dir(directory, ambient_authority()) {
        Ok(handle) => handle,
        Err(error) => panic!("manifest directory {directory} should open: {error}"),
    };
    match handle.read_to_string(filename) {
        Ok(contents) => contents,
        Err(error) => panic!("manifest should be readable at {path}: {error}"),
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

/// Locates a crate's manifest relative to this test crate, or reports absence.
///
/// The workspace lays its crates out as `<root>/crates/<crate>/Cargo.toml`, and
/// this test lives in `crates/whitaker_sarif`, so the current crate is found at
/// `CARGO_MANIFEST_DIR` and a sibling beside it.
///
/// Returning an `Option` rather than panicking is what lets the mapping-crate
/// half of the guard stay dormant until ADR 005's mapping crate is created.
pub(crate) fn manifest_if_present(crate_name: &str) -> Option<Utf8PathBuf> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.file_name() == Some(crate_name) {
        return Some(manifest_dir.join("Cargo.toml"));
    }
    let candidate = manifest_dir
        .parent()
        .map(|parent| parent.join(crate_name).join("Cargo.toml"))?;
    candidate.is_file().then_some(candidate)
}
