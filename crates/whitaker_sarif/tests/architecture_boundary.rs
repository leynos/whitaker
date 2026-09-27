//! Architecture-fitness guard for the brain trust lint driver seam.
//!
//! ADR 005 (`docs/adr-005-brain-trust-lint-driver-interfaces.md`) fixes the
//! dependency direction across the brain trust lint driver seam: `whitaker_sarif`
//! stays a pure SARIF model and must not acquire `whitaker-common`, and the
//! SARIF mapping crate must not name the localization stack.
//!
//! Both halves of the rule are enforced against a real manifest whenever one
//! exists. The `whitaker_sarif` half always does, so a reintroduced
//! `whitaker-common` edge fails today. The mapping crate does not exist yet —
//! ADR 005 records the interface *before* any consumer is implemented — so that
//! half carries a dormant test that activates the moment the crate lands, and
//! fixture cases that verify the rule in the meantime.
//!
//! The rule is stated over direct dependency names rather than over
//! reachability: the mapping crate legitimately depends on `whitaker-common`,
//! whose `i18n` module cannot be feature-gated out, so what the guard can
//! enforce is that the mapping crate's own manifest stays informative.
//!
//! A dependency's effective name is not always written in the manifest that
//! declares it. A member may rename an edge locally (`loc = { package = … }`)
//! or inherit a rename from the workspace (`loc = { workspace = true }`), and
//! the guard resolves all three shapes. The inherited one needs the root
//! `[workspace.dependencies]` table, which is read alongside every real
//! manifest; an inherited entry whose declaration cannot be read fails closed
//! rather than passing as if the edge were absent.
//!
//! Precedent: `crates/whitaker_clones_core/build_support.rs` parses a manifest
//! with `toml::Table` and walks its dependency tables.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;

/// The crate that must not acquire `whitaker-common`.
const SARIF_CRATE: &str = "whitaker_sarif";

/// The crate that must not acquire the localization stack.
const MAPPING_CRATE: &str = "whitaker_brain_trust_sarif";

/// The dependency `SARIF_CRATE` must not name.
const FORBIDDEN_IN_SARIF: &str = "whitaker-common";

/// The dependencies `MAPPING_CRATE` must not name.
const LOCALIZATION_STACK: [&str; 2] = ["fluent-templates", "unic-langid"];

/// The dependency tables a guard inspects.
///
/// `[dependencies]` is the production edge ADR 005 governs. The dev and build
/// tables are inspected too, because a cycle is a cycle whichever table carries
/// it, and because a dev-dependency on `whitaker-common` would put the
/// localization stack into the dependent crate's test build.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// A dependency table's path within the manifest, and its entries.
type DependencyTable = (String, toml::Table);

/// The outcome of scanning a manifest for one package name.
#[derive(Debug, PartialEq, Eq)]
enum ScanOutcome {
    /// The package was declared; the payload is the table and key that named it.
    Found(String),
    /// The package was not declared.
    Absent,
}

impl ScanOutcome {
    /// Returns whether the package was declared.
    #[must_use]
    const fn is_found(&self) -> bool { matches!(self, Self::Found(_)) }

    /// Returns the declaring location, or `"nowhere"` when absent.
    #[must_use]
    fn location(&self) -> &str {
        match self {
            Self::Found(where_) => where_,
            Self::Absent => "nowhere",
        }
    }
}

/// Returns whether a dependency entry names `package`.
///
/// An entry can name its package in three places, and all three are read.
///
/// 1. The key: `whitaker-common = ...`.
/// 2. A local `package` field under a rename: `loc = { package = "whitaker-common" }` declares the
///    same forbidden edge as the key form while leaving the key innocuous, so a key-only scan would
///    pass it.
/// 3. The workspace's own declaration, reached through `{ workspace = true }`. A member may inherit
///    a rename without writing `package` locally — `loc = { workspace = true }` — and the name then
///    lives only in the root `[workspace.dependencies]` entry for `loc`, under a `package` field
///    or, absent one, in the key itself. This is the one shape a member manifest cannot be read in
///    isolation to resolve.
///
/// `workspace_dependencies` is the root workspace's `[workspace.dependencies]`
/// table, or `None` when the manifest under scan *is* that root, which has no
/// parent to inherit from. An inherited entry whose declaration is absent or
/// unreadable is treated as unresolvable rather than silently passed, so the
/// guard fails closed.
fn names_package(
    key: &str,
    value: &toml::Value,
    package: &str,
    workspace_dependencies: Option<&toml::Table>,
) -> bool {
    if key == package
        || value
            .get("package")
            .and_then(toml::Value::as_str)
            .is_some_and(|named| named == package)
    {
        return true;
    }

    let inherits = value
        .get("workspace")
        .and_then(toml::Value::as_bool)
        .is_some_and(|flag| flag);
    if !inherits {
        return false;
    }

    workspace_dependencies
        .and_then(|dependencies| dependencies.get(key))
        .is_some_and(|declaration| {
            declaration
                .get("package")
                .and_then(toml::Value::as_str)
                .unwrap_or(key)
                == package
        })
}

/// Joins a table path prefix to a key, omitting an empty prefix.
fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Recursively harvests dependency tables, tracking the path that reached each.
///
/// Recursion is required rather than a top-level scan: a `[target.'cfg(…)'…]`
/// selector nests its dependency tables one or more levels down, and an edge can
/// hide there.
fn collect_tables(node: &toml::Table, prefix: &str, into: &mut Vec<DependencyTable>) {
    for (key, value) in node {
        let Some(table) = value.as_table() else {
            continue;
        };
        let path = join_path(prefix, key);
        if DEPENDENCY_TABLES.iter().any(|name| name == key) {
            into.push((path.clone(), table.clone()));
        }
        collect_tables(table, &path, into);
    }
}

/// Collects every dependency table a manifest declares.
fn dependency_tables(document: &toml::Table) -> Vec<DependencyTable> {
    let mut tables = Vec::new();
    collect_tables(document, "", &mut tables);
    tables
}

/// Scans a manifest for a dependency, naming where it was found.
///
/// `workspace_dependencies` is the root workspace's `[workspace.dependencies]`
/// table, if the manifest under scan inherits from one. Fixture manifests pass
/// `None`: they exercise the local shapes, and the inherited shape is covered
/// separately by `inherited_rename_is_resolved_through_the_workspace`.
fn scan_for(
    document: &toml::Table,
    package: &str,
    workspace_dependencies: Option<&toml::Table>,
) -> ScanOutcome {
    for (table, entries) in dependency_tables(document) {
        for (key, value) in &entries {
            if names_package(key, value, package, workspace_dependencies) {
                return ScanOutcome::Found(format!("{table}.{key}"));
            }
        }
    }
    ScanOutcome::Absent
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
fn workspace_dependencies() -> Option<toml::Table> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent()?.parent()?.join("Cargo.toml");
    if !root.is_file() {
        return None;
    }
    let document = parse_manifest(&read_manifest(&root));
    document
        .get("workspace")?
        .get("dependencies")?
        .as_table()
        .cloned()
}

/// Parses a manifest into a TOML table.
///
/// # Panics
///
/// Panics when the manifest is not valid TOML. Every input is either a tracked
/// file or a fixture literal, so a parse failure is a defect in this test rather
/// than a condition under test.
fn parse_manifest(manifest: &str) -> toml::Table {
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
fn read_manifest(path: &Utf8Path) -> String {
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

/// Locates a crate's manifest relative to this test crate, or reports absence.
///
/// The workspace lays its crates out as `<root>/crates/<crate>/Cargo.toml`, and
/// this test lives in `crates/whitaker_sarif`, so the current crate is found at
/// `CARGO_MANIFEST_DIR` and a sibling beside it.
///
/// Returning an `Option` rather than panicking is what lets the mapping-crate
/// half of the guard stay dormant until ADR 005's mapping crate is created.
fn manifest_if_present(crate_name: &str) -> Option<Utf8PathBuf> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.file_name() == Some(crate_name) {
        return Some(manifest_dir.join("Cargo.toml"));
    }
    let candidate = manifest_dir
        .parent()
        .map(|parent| parent.join(crate_name).join("Cargo.toml"))?;
    candidate.is_file().then_some(candidate)
}

// -- The rule, over the real manifest ---------------------------------------

#[rstest]
fn sarif_crate_does_not_depend_on_whitaker_common() {
    // Resolved here rather than in a helper: `whitaker_sarif` is this test
    // crate's sibling by construction, and the repo denies `expect()` outside
    // a test body, so the failure belongs at the call site it can be read at.
    let manifest = manifest_if_present(SARIF_CRATE)
        .expect("whitaker_sarif is a sibling of the test crate that must find it");
    let document = parse_manifest(&read_manifest(&manifest));

    let tables = dependency_tables(&document);
    let entries: usize = tables.iter().map(|(_, table)| table.len()).sum();

    // Non-vacuity floor: a path typo, a renamed table, or a restructure must
    // not be able to make this guard pass by examining nothing.
    assert!(
        !tables.is_empty(),
        "no dependency table found in {SARIF_CRATE}/Cargo.toml; the guard is vacuous"
    );
    assert!(
        entries > 0,
        "no dependency examined in {SARIF_CRATE}/Cargo.toml; the guard is vacuous"
    );

    // The workspace table is resolved too, so a member that inherited the
    // forbidden edge through `{ workspace = true }` is still caught.
    let outcome = scan_for(
        &document,
        FORBIDDEN_IN_SARIF,
        workspace_dependencies().as_ref(),
    );
    assert!(
        !outcome.is_found(),
        "{SARIF_CRATE} must not depend on {FORBIDDEN_IN_SARIF}, but declares it at {}",
        outcome.location()
    );
}

// -- The rule, over the mapping crate, which does not exist yet -------------

/// A clean mapping-crate manifest.
///
/// It depends on `whitaker-common` legitimately, because `SubjectLocation`
/// carries a `RepoRelativePath` and a `SourceSpan`. That edge is required, not
/// forbidden; only the localization stack is.
const MAPPING_CLEAN: &str = concat!(
    "[package]\n",
    "name = \"whitaker_brain_trust_sarif\"\n",
    "version = \"0.1.0\"\n",
    "publish = false\n",
    "\n",
    "[dependencies]\n",
    "whitaker-common = { workspace = true }\n",
    "whitaker_sarif = { workspace = true }\n",
);

/// The forbidden shape: the localization stack named outright.
const MAPPING_DIRECT: &str = concat!(
    "[dependencies]\n",
    "whitaker-common = { workspace = true }\n",
    "fluent-templates = { workspace = true }\n",
);

/// The forbidden shape under a rename, which a key-only scan would pass.
const MAPPING_RENAMED: &str = concat!(
    "[dependencies]\n",
    "loc = { package = \"fluent-templates\", version = \"0.15\" }\n",
);

/// The forbidden shape hidden behind a target selector.
const MAPPING_TARGET_GATED: &str = concat!(
    "[target.'cfg(unix)'.dependencies]\n",
    "unic-langid = \"0.9\"\n",
);

/// The forbidden shape in a non-production table.
const MAPPING_DEV_DEPENDENCY: &str = concat!(
    "[dev-dependencies]\n",
    "fluent-templates = { workspace = true }\n",
);

/// The forbidden shape reached through a workspace dependency entry.
const MAPPING_WORKSPACE_INLINE: &str = concat!(
    "[dependencies]\n",
    "unic-langid = { workspace = true, optional = true }\n",
);

#[rstest]
#[case::clean(MAPPING_CLEAN, None)]
#[case::direct(MAPPING_DIRECT, Some("fluent-templates"))]
#[case::renamed(MAPPING_RENAMED, Some("fluent-templates"))]
#[case::target_gated(MAPPING_TARGET_GATED, Some("unic-langid"))]
#[case::dev_dependency(MAPPING_DEV_DEPENDENCY, Some("fluent-templates"))]
#[case::workspace_inline(MAPPING_WORKSPACE_INLINE, Some("unic-langid"))]
fn mapping_crate_must_not_name_the_localization_stack(
    #[case] manifest: &str,
    #[case] expected: Option<&str>,
) {
    let document = parse_manifest(manifest);
    let found = LOCALIZATION_STACK
        .iter()
        .find(|package| scan_for(&document, package, None).is_found())
        .copied();

    assert_eq!(
        found, expected,
        "fixture verdict disagrees with the rule for {MAPPING_CRATE}"
    );
}

// -- Non-vacuity: the scan reads renames, not only keys ---------------------

#[rstest]
#[case::direct("[dependencies]\nwhitaker-common = { workspace = true }\n")]
#[case::renamed("[dependencies]\nloc = { package = \"whitaker-common\" }\n")]
#[case::workspace_inline(
    "[dependencies]\nloc = { workspace = true, package = \"whitaker-common\" }\n"
)]
fn sarif_rule_rejects_both_dependency_shapes(#[case] manifest: &str) {
    let document = parse_manifest(manifest);
    let outcome = scan_for(&document, FORBIDDEN_IN_SARIF, None);
    assert!(
        outcome.is_found(),
        "a {FORBIDDEN_IN_SARIF} edge must be rejected in every shape"
    );
}

#[rstest]
fn scan_reports_the_declaring_key() {
    let document = parse_manifest(MAPPING_RENAMED);
    assert_eq!(
        scan_for(&document, "fluent-templates", None).location(),
        "dependencies.loc",
        "the failure must name the key that declared the edge"
    );
}

#[rstest]
fn scan_reports_absence_without_claiming_a_location() {
    let document = parse_manifest("[dependencies]\nserde = \"1\"\n");
    assert_eq!(
        scan_for(&document, FORBIDDEN_IN_SARIF, None),
        ScanOutcome::Absent,
        "an absent dependency must not be reported as declared"
    );
}

// -- Non-vacuity: an inherited rename is resolved through the workspace -----

/// A member manifest that inherits a rename without naming the package.
///
/// This is the shape `names_package` cannot resolve from the member alone: the
/// key is `loc`, there is no local `package`, and the real name lives only in
/// the root workspace's entry for `loc`.
const INHERITED_RENAME: &str = concat!(
    "[package]\n",
    "name = \"whitaker_brain_trust_sarif\"\n",
    "\n",
    "[dependencies]\n",
    "loc = { workspace = true }\n",
);

/// The root table that makes `INHERITED_RENAME` name `fluent-templates`.
const WORKSPACE_WITH_RENAME: &str = concat!(
    "[workspace.dependencies]\n",
    "loc = { package = \"fluent-templates\", version = \"0.15\" }\n",
);

#[rstest]
fn inherited_rename_is_resolved_through_the_workspace() {
    let document = parse_manifest(INHERITED_RENAME);
    let workspace = parse_manifest(WORKSPACE_WITH_RENAME);
    let dependencies = workspace
        .get("workspace")
        .and_then(|workspace_table| workspace_table.get("dependencies"))
        .and_then(toml::Value::as_table);

    assert!(
        dependencies.is_some(),
        "the fixture must declare a workspace dependency table"
    );

    // Read in isolation the edge is invisible, which is what makes this case
    // worth pinning: the local manifest names no forbidden package anywhere.
    assert!(
        !scan_for(&document, "fluent-templates", None).is_found(),
        "the member manifest alone cannot resolve an inherited rename"
    );

    // With the workspace table the effective name is recovered.
    let outcome = scan_for(&document, "fluent-templates", dependencies);
    assert!(
        outcome.is_found(),
        "an inherited rename must resolve to the package the workspace declares"
    );
    assert_eq!(
        outcome.location(),
        "dependencies.loc",
        "the failure must name the member key that inherited the edge"
    );
}

#[rstest]
fn inherited_entry_without_a_workspace_declaration_is_not_treated_as_clean() {
    // A member that inherits a key the workspace does not declare cannot be
    // resolved. Failing closed is the point: a False here would let an
    // unreadable declaration read as an absent dependency.
    let document = parse_manifest("[dependencies]\nloc = { workspace = true }\n");
    let empty = parse_manifest("[workspace.dependencies]\nserde = \"1\"\n");
    let dependencies = empty
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table);

    assert!(
        !scan_for(&document, "fluent-templates", dependencies).is_found(),
        "an unresolvable inherited entry must not be reported as declared"
    );
    assert!(
        !scan_for(&document, "serde", dependencies).is_found(),
        "a key absent from the member manifest is absent regardless of the workspace"
    );
}

#[rstest]
fn a_plain_key_is_never_resolved_through_the_workspace() {
    // The workspace lookup applies only to `{ workspace = true }` entries. A
    // local key that happens to match a workspace key with a different package
    // must not pick up the workspace's name.
    let document = parse_manifest("[dependencies]\nloc = \"1\"\n");
    let workspace = parse_manifest(WORKSPACE_WITH_RENAME);
    let dependencies = workspace
        .get("workspace")
        .and_then(|workspace_table| workspace_table.get("dependencies"))
        .and_then(toml::Value::as_table);

    assert!(
        !scan_for(&document, "fluent-templates", dependencies).is_found(),
        "a non-inherited entry keeps its own name"
    );
}

// -- The rule, over the mapping crate's real manifest once it exists --------

#[rstest]
fn mapping_crate_manifest_is_bound_when_it_exists() {
    let Some(path) = manifest_if_present(MAPPING_CRATE) else {
        // Not yet created. ADR 005 records the interface before any consumer
        // exists, so this is the expected state until the mapping crate lands;
        // the fixture tests above are what verify the rule in the meantime.
        return;
    };

    let document = parse_manifest(&read_manifest(&path));

    // Once the crate exists the guard must not be able to pass by examining
    // nothing: a manifest with no dependency table at all cannot evidence the
    // absence of an edge, so report that as a failure rather than a pass.
    let tables = dependency_tables(&document);
    assert!(
        !tables.is_empty(),
        "{MAPPING_CRATE}/Cargo.toml declares no dependency table; the guard is vacuous"
    );
    let entries: usize = tables.iter().map(|(_, table)| table.len()).sum();
    assert!(
        entries > 0,
        "{MAPPING_CRATE}/Cargo.toml declares no dependency; the guard is vacuous"
    );

    // The workspace table is resolved here too, so the mapping crate cannot
    // inherit the localization stack through `{ workspace = true }`.
    let workspace = workspace_dependencies();
    for package in LOCALIZATION_STACK {
        let outcome = scan_for(&document, package, workspace.as_ref());
        assert!(
            !outcome.is_found(),
            "{MAPPING_CRATE} must not name {package}, but declares it at {}",
            outcome.location()
        );
    }
}

/// The discovery path above must not be able to silently skip the guard.
///
/// `mapping_crate_manifest_is_bound_when_it_exists` returns early today, so its
/// own body cannot show that discovery works. These two cases pin both branches
/// against crates that are present and absent *now*, so a later regression in
/// `manifest_if_present` cannot make the guard quietly vacuous once the mapping
/// crate lands.
#[rstest]
#[case::present("whitaker_clones_core", true)]
#[case::absent("whitaker_no_such_crate", false)]
fn manifest_discovery_finds_siblings_that_exist(#[case] crate_name: &str, #[case] expected: bool) {
    assert_eq!(
        manifest_if_present(crate_name).is_some(),
        expected,
        "discovery of {crate_name} disagrees with the filesystem"
    );
}
