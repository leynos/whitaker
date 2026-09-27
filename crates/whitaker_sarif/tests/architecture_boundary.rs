//! Architecture-fitness guard for the brain trust lint driver seam.
//!
//! ADR 005 (`docs/adr-005-brain-trust-lint-driver-interfaces.md`) fixes the
//! dependency direction across the brain trust lint driver seam: `whitaker_sarif`
//! stays a pure SARIF model and must not acquire `whitaker-common`, and the
//! SARIF mapping crate must not acquire the localization stack.
//!
//! The mapping crate does not exist yet — ADR 005 records the interface
//! *before* any consumer is implemented — so that half of the rule runs against
//! fixture manifests. It verifies the rule and gains teeth when the crate lands.
//! The `whitaker_sarif` half runs against the real manifest, so a reintroduced
//! `whitaker-common` edge fails today.
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
/// A dependency names its package either in the key or, when renamed, in a
/// `package` field. Both are read: `loc = { package = "whitaker-common" }`
/// declares the same forbidden edge as `whitaker-common = ...` while leaving the
/// key innocuous, so a key-only scan would pass it.
fn names_package(key: &str, value: &toml::Value, package: &str) -> bool {
    key == package
        || value
            .get("package")
            .and_then(toml::Value::as_str)
            .is_some_and(|named| named == package)
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
fn scan_for(document: &toml::Table, package: &str) -> ScanOutcome {
    for (table, entries) in dependency_tables(document) {
        for (key, value) in &entries {
            if names_package(key, value, package) {
                return ScanOutcome::Found(format!("{table}.{key}"));
            }
        }
    }
    ScanOutcome::Absent
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

    let outcome = scan_for(&document, FORBIDDEN_IN_SARIF);
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
        .find(|package| scan_for(&document, package).is_found())
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
    let outcome = scan_for(&document, FORBIDDEN_IN_SARIF);
    assert!(
        outcome.is_found(),
        "a {FORBIDDEN_IN_SARIF} edge must be rejected in every shape"
    );
}

#[rstest]
fn scan_reports_the_declaring_key() {
    let document = parse_manifest(MAPPING_RENAMED);
    assert_eq!(
        scan_for(&document, "fluent-templates").location(),
        "dependencies.loc",
        "the failure must name the key that declared the edge"
    );
}

#[rstest]
fn scan_reports_absence_without_claiming_a_location() {
    let document = parse_manifest("[dependencies]\nserde = \"1\"\n");
    assert_eq!(
        scan_for(&document, FORBIDDEN_IN_SARIF),
        ScanOutcome::Absent,
        "an absent dependency must not be reported as declared"
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

    for package in LOCALIZATION_STACK {
        let outcome = scan_for(&document, package);
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
