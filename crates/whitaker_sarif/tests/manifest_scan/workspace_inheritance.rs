//! Behavioural tests for workspace inheritance in the manifest scan.
//!
//! A member manifest can inherit a dependency — and a rename — through
//! `{ workspace = true }`, in which case the package's real name lives only in
//! the root `[workspace.dependencies]` table. These tests pin how the scan
//! resolves that third shape, and the two failure modes it must not have:
//! reading an unreadable entry as absent, and letting one decide the scan
//! before a later entry has been visited.
//!
//! They sit here rather than in `architecture_boundary.rs` so that file stays a
//! statement of ADR 005's rule. These are statements about the scanner.

use rstest::rstest;

use super::{ScanOutcome, parse_manifest, scan_for, workspace_table_of};

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
    let dependencies = workspace_table_of(&parse_manifest(WORKSPACE_WITH_RENAME));

    // Read in isolation the edge is invisible, which is what makes this case
    // worth pinning: the local manifest names no forbidden package anywhere.
    // It is unresolved rather than absent — the key inherits, and there is no
    // workspace table here to resolve it against.
    assert_eq!(
        scan_for(&document, "fluent-templates", None),
        ScanOutcome::Unresolved("dependencies.loc".to_owned()),
        "the member manifest alone cannot resolve an inherited rename"
    );

    // With the workspace table the effective name is recovered.
    assert_eq!(
        scan_for(&document, "fluent-templates", dependencies.as_ref()),
        ScanOutcome::Found("dependencies.loc".to_owned()),
        "an inherited rename must resolve to the package the workspace declares"
    );
}

#[rstest]
fn inherited_entry_without_a_workspace_declaration_is_not_treated_as_clean() {
    // A member that inherits a key the workspace does not declare cannot be
    // resolved. Failing closed is the point: an `Absent` here would let an
    // unreadable declaration read as a missing dependency, and the guard would
    // pass while blind to whatever that entry's name actually is.
    let document = parse_manifest("[dependencies]\nloc = { workspace = true }\n");
    // A workspace table that declares some *other* key, so the inheriting entry
    // resolves to nothing and the scan cannot read its name.
    let dependencies =
        workspace_table_of(&parse_manifest("[workspace.dependencies]\nserde = \"1\"\n"));

    let outcome = scan_for(&document, "fluent-templates", dependencies.as_ref());
    assert_eq!(
        outcome,
        ScanOutcome::Unresolved("dependencies.loc".to_owned()),
        "an unresolvable inherited entry must be reported, not read as absent"
    );
    assert!(
        !outcome.is_absent(),
        "an unresolved scan must fail the guard rather than certify absence"
    );

    // The unresolved entry taints every package scanned, not only the one it
    // happens to name. `loc` could carry any name, so no scan of this manifest
    // can certify an absence while it is unreadable. The result is a failure
    // rather than a pass, which is the point of the rule.
    assert_eq!(
        scan_for(&document, "serde", dependencies.as_ref()),
        ScanOutcome::Unresolved("dependencies.loc".to_owned()),
        "an unreadable entry leaves every package unproven, not only its own"
    );

    // A manifest with no inheriting entry is scanned to completion, so a
    // genuine absence still reports as one.
    let plain = parse_manifest("[dependencies]\nserde = \"1\"\n");
    assert_eq!(
        scan_for(&plain, "fluent-templates", dependencies.as_ref()),
        ScanOutcome::Absent,
        "without an unreadable entry, absence is certified as absence"
    );
}

#[rstest]
fn an_unresolved_entry_does_not_mask_a_declared_one() {
    // The fail-closed rule must not turn a genuine finding into a failure to
    // resolve: an explicit declaration outranks an unrelated unreadable entry,
    // so the guard still names the edge it found.
    //
    // The unreadable key sorts *before* the declaring one, so a scan that
    // returned on the first unresolved entry would report `Unresolved` here.
    // `toml::Table` iterates in key order, so the ordering is deliberately
    // adversarial rather than incidental.
    //
    // The declaring key is written plainly rather than inherited: Cargo *errors*
    // on an inheriting entry the root does not declare, so a member written as
    // `fluent-templates = { workspace = true }` beside an undeclared `aaa`
    // could never be loaded. The unresolvable entry and the resolvable one have
    // to come from different shapes for this manifest to be one Cargo accepts.
    let document = parse_manifest(concat!(
        "[dependencies]\n",
        "aaa = { workspace = true }\n",
        "fluent-templates = \"0.15\"\n",
    ));
    let dependencies =
        workspace_table_of(&parse_manifest("[workspace.dependencies]\nserde = \"1\"\n"));
    // The workspace declares neither key, so the found entry is named by its own
    // key and owes nothing to the workspace resolution.
    assert_eq!(
        scan_for(&document, "fluent-templates", dependencies.as_ref()),
        ScanOutcome::Found("dependencies.fluent-templates".to_owned()),
        "a declared entry must be reported even beside an unresolvable one"
    );
}

#[rstest]
fn a_plain_key_is_never_resolved_through_the_workspace() {
    // The workspace lookup applies only to `{ workspace = true }` entries. A
    // local key that happens to match a workspace key with a different package
    // must not pick up the workspace's name.
    let document = parse_manifest("[dependencies]\nloc = \"1\"\n");
    let dependencies = workspace_table_of(&parse_manifest(WORKSPACE_WITH_RENAME));

    // The local entry does not inherit, so the workspace's name for `loc` is
    // irrelevant to it. A scan that consulted the workspace anyway would find
    // `fluent-templates` here and report a false positive.
    assert_eq!(
        scan_for(&document, "fluent-templates", dependencies.as_ref()),
        ScanOutcome::Absent,
        "a non-inherited entry keeps its own name"
    );
}

#[rstest]
fn an_inheriting_entry_is_named_by_the_root_not_by_its_key() {
    // The member's key is the *local* name; an inheriting entry's identity is
    // the root declaration's. Cargo proves the order is inheritance-first in
    // both directions, so both are pinned here.
    //
    // Verified against `cargo metadata`: with a root of
    // `whitaker-common = { package = "pkg_a", path = "…" }` and a member
    // declaring `whitaker-common = { workspace = true }`, Cargo reports
    // `name: pkg_a | rename: whitaker-common` — the key is not the package.
    // A scan that matched the key first would fail the guard on that manifest
    // for an edge Cargo does not have.
    let document = parse_manifest("[dependencies]\nwhitaker-common = { workspace = true }\n");
    let dependencies = workspace_table_of(&parse_manifest(concat!(
        "[workspace.dependencies]\n",
        "whitaker-common = { package = \"serde\", version = \"1\" }\n",
    )));

    assert_eq!(
        scan_for(&document, "serde", dependencies.as_ref()),
        ScanOutcome::Found("dependencies.whitaker-common".to_owned()),
        "an inherited entry is the package the root declares, under a local key"
    );
    assert_eq!(
        scan_for(&document, "whitaker-common", dependencies.as_ref()),
        ScanOutcome::Absent,
        "the local key is not the inherited entry's identity"
    );

    // A local `package` written beside `workspace = true` does not override the
    // inherited identity. Cargo resolves the root's package and warns
    // `unused manifest key: dependencies.<key>.package`; the member's own value
    // is discarded outright. Reading it would let a member hide a forbidden
    // edge behind a permissible root declaration, or the reverse.
    let shadowed = parse_manifest(concat!(
        "[dependencies]\n",
        "whitaker-common = { workspace = true, package = \"serde\" }\n",
    ));
    assert_eq!(
        scan_for(&shadowed, "whitaker-common", dependencies.as_ref()),
        ScanOutcome::Absent,
        "a discarded local `package` must not be read as the identity"
    );
}
