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
//! declares it, so an edge can hide behind a rename or behind `{ workspace =
//! true }`. `manifest_scan` resolves all three shapes and fails closed on an
//! entry whose name it cannot read; this file is the rule those scans serve.

mod manifest_scan;

use manifest_scan::{
    ScanOutcome,
    assert_non_vacuous,
    manifest_if_present,
    parse_manifest,
    read_manifest,
    scan_for,
    workspace_dependencies,
    workspace_table_of,
};
use rstest::rstest;

/// The crate that must not acquire `whitaker-common`.
const SARIF_CRATE: &str = "whitaker_sarif";

/// The crate that must not acquire the localization stack.
const MAPPING_CRATE: &str = "whitaker_brain_trust_sarif";

/// The dependency `SARIF_CRATE` must not name.
const FORBIDDEN_IN_SARIF: &str = "whitaker-common";

/// The dependencies `MAPPING_CRATE` must not name.
const LOCALIZATION_STACK: [&str; 2] = ["fluent-templates", "unic-langid"];

// -- The rule, over the real manifest ---------------------------------------

#[rstest]
fn sarif_crate_does_not_depend_on_whitaker_common() {
    // Resolved here rather than in a helper: `whitaker_sarif` is this test
    // crate's sibling by construction, and the repo denies `expect()` outside
    // a test body, so the failure belongs at the call site it can be read at.
    let manifest = manifest_if_present(SARIF_CRATE)
        .expect("whitaker_sarif is a sibling of the test crate that must find it");
    let document = parse_manifest(&read_manifest(&manifest));
    assert_non_vacuous(&document, SARIF_CRATE);

    // The workspace table is resolved too, so a member that inherited the
    // forbidden edge through `{ workspace = true }` is still caught.
    let outcome = scan_for(
        &document,
        FORBIDDEN_IN_SARIF,
        workspace_dependencies().as_ref(),
    );
    // `is_absent` rather than `!is_found`: an entry this scan could not read is
    // not evidence that the edge is gone, so it must fail the guard too.
    assert!(
        outcome.is_absent(),
        "{SARIF_CRATE} must not depend on {FORBIDDEN_IN_SARIF}, but {}",
        outcome.finding()
    );
}

// -- The rule, over the mapping crate, which does not exist yet -------------

/// A clean mapping-crate manifest.
///
/// It depends on `whitaker-common` legitimately: the value crossing the seam is
/// `whitaker_common::paths::FindingLocation`, which pairs a `RepoRelativePath`
/// with a `SourceSpan` and carries no compiler type. That edge is required, not
/// forbidden; only the localization stack is. `SubjectLocation` stays above this
/// seam, in the tier that can name `rustc_hir::HirId`, and the mapping crate
/// never sees it.
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

/// The workspace table the inheriting fixtures are resolved against.
///
/// A fixture models a member manifest, and a member always has a root. Leaving
/// its `{ workspace = true }` entries unreadable would fail each case for the
/// fallback rather than the rule, so every inherited key is declared here.
const FIXTURE_WORKSPACE: &str = concat!(
    "[workspace.dependencies]\n",
    "whitaker-common = { path = \"crates/whitaker-common\" }\n",
    "whitaker_sarif = { path = \"crates/whitaker_sarif\" }\n",
    "fluent-templates = { path = \"vendor/fluent-templates\" }\n",
    "unic-langid = { path = \"vendor/unic-langid\" }\n",
);

/// A fixture inheriting an entry no workspace declaration resolves, which is the
/// fail-closed case at fixture level: the key might name either package, so
/// neither is certified absent and the fixture must not read as clean.
const MAPPING_UNREADABLE: &str = concat!("[dependencies]\n", "loc = { workspace = true }\n",);

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
#[case::clean(MAPPING_CLEAN, Some(FIXTURE_WORKSPACE), None)]
#[case::direct(MAPPING_DIRECT, Some(FIXTURE_WORKSPACE), Some("fluent-templates"))]
#[case::renamed(MAPPING_RENAMED, Some(FIXTURE_WORKSPACE), Some("fluent-templates"))]
#[case::target_gated(MAPPING_TARGET_GATED, Some(FIXTURE_WORKSPACE), Some("unic-langid"))]
#[case::dev_dependency(
    MAPPING_DEV_DEPENDENCY,
    Some(FIXTURE_WORKSPACE),
    Some("fluent-templates")
)]
#[case::workspace_inline(MAPPING_WORKSPACE_INLINE, Some(FIXTURE_WORKSPACE), Some("unic-langid"))]
// The one case that passes no workspace: `unreadable_inheritance` is *about*
// the fallback rather than about the rule.
#[case::unreadable_inheritance(MAPPING_UNREADABLE, None, Some("fluent-templates"))]
fn mapping_crate_must_not_name_the_localization_stack(
    #[case] manifest: &str,
    #[case] workspace: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let document = parse_manifest(manifest);
    let root = workspace.map(parse_manifest);
    let dependencies = root.as_ref().and_then(workspace_table_of);

    // Fail-closed here too, and for the same reason as the real guards: only a
    // package the scan could *read* and found counts as a finding, and an entry
    // that could not be read must not be read as clean. `is_found()` alone
    // would report `Unresolved` as clean, which is the gap this closes.
    let found = LOCALIZATION_STACK
        .iter()
        .find(|package| !scan_for(&document, package, dependencies.as_ref()).is_absent())
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
        scan_for(&document, "fluent-templates", None),
        ScanOutcome::Found("dependencies.loc".to_owned()),
        "the failure must name the key that declared the edge"
    );
}

#[rstest]
fn a_package_override_displaces_the_key_as_the_identity() {
    // `whitaker-common = { package = "serde" }` depends on `serde`; Cargo
    // resolves the `package` field when one is written and the key only when
    // none is. Verified against `cargo metadata`, which reports `name: serde`
    // with `rename: whitaker-common` for exactly this shape.
    //
    // So this manifest declares no `whitaker-common` edge, and a scan reading
    // the key as the identity would fail the guard on a clean manifest. The
    // mirror of `MAPPING_RENAMED`, which hides a forbidden edge behind an
    // innocuous key; this one hides an innocuous package behind a forbidden
    // key, and the two together are what pin the resolution order.
    let document = parse_manifest(
        "[dependencies]\nwhitaker-common = { package = \"serde\", version = \"1\" }\n",
    );
    assert_eq!(
        scan_for(&document, FORBIDDEN_IN_SARIF, None),
        ScanOutcome::Absent,
        "the `package` field, not the key, is the dependency's identity"
    );

    // The override is not a blanket exemption: when it *does* name the
    // forbidden package the edge is found, which is what `sarif_rule_rejects_
    // both_dependency_shapes` covers under `case::renamed`.
    assert_eq!(
        scan_for(&document, "serde", None),
        ScanOutcome::Found("dependencies.whitaker-common".to_owned()),
        "an override that names another package makes that package the edge"
    );
}

#[rstest]
fn metadata_tables_are_not_dependency_tables() {
    // Cargo reads dependencies only at the manifest top level, under a
    // `[target.<cfg>]` selector, and under `[workspace]`. It never resolves
    // `package.metadata`, so a `dependencies` key there is arbitrary data a
    // tool chose to store — not an edge. Harvesting it would fail the guard on
    // a manifest that declares no forbidden dependency at all.
    let document = parse_manifest(concat!(
        "[package]\n",
        "name = \"whitaker_sarif\"\n",
        "\n",
        "[dependencies]\n",
        "serde = \"1\"\n",
        "\n",
        "[package.metadata.tool.dependencies]\n",
        "fluent-templates = { version = \"0.15\" }\n",
    ));
    assert_eq!(
        scan_for(&document, "fluent-templates", None),
        ScanOutcome::Absent,
        "a metadata table is not a Cargo dependency table"
    );

    // The genuine table in the same manifest is still read, so this is a
    // scoping fix rather than a disabled scan.
    assert!(
        scan_for(&document, "serde", None).is_found(),
        "the real `[dependencies]` table must still be scanned"
    );
}

#[rstest]
fn workspace_dependencies_table_is_scanned_too() {
    // `[workspace.dependencies]` is the third place Cargo reads dependencies,
    // and it is a real declaration rather than metadata: an entry there is what
    // every member inheriting `{ workspace = true }` resolves against. A scan
    // that read only the top level and `[target]` would miss a forbidden edge
    // declared at the root and shared by every member.
    //
    // Without this case the `workspace` clause of `is_dependency_scope` is
    // unreachable: every other test passes the workspace table as the
    // *resolution* argument to `scan_for`, never as the document under scan.
    // Dropping the clause left all 105 tests passing, which is what makes this
    // case load-bearing rather than decorative.
    let document = parse_manifest(concat!(
        "[workspace.dependencies]\n",
        "fluent-templates = { version = \"0.15\" }\n",
    ));
    assert_eq!(
        scan_for(&document, "fluent-templates", None),
        ScanOutcome::Found("workspace.dependencies.fluent-templates".to_owned()),
        "a root workspace declaration is a dependency the guard must read"
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

// Workspace-inheritance behaviour is tested in
// `manifest_scan/workspace_inheritance.rs`, beside the scanner it exercises.

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

    // A manifest with no dependency table cannot evidence the absence of an
    // edge, so the non-vacuity floor applies here too.
    assert_non_vacuous(&document, MAPPING_CRATE);

    let workspace = workspace_dependencies();
    for package in LOCALIZATION_STACK {
        let outcome = scan_for(&document, package, workspace.as_ref());
        assert!(
            outcome.is_absent(),
            "{MAPPING_CRATE} must not name {package}, but {}",
            outcome.finding()
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
