//! Behavioural tests for which tables the manifest scan treats as dependency
//! tables.
//!
//! A dependency is read only where Cargo reads one, and Cargo's grammar is
//! narrower than "any table named `dependencies`". These tests pin that
//! grammar, because the failure mode of widening it is a false positive: the
//! guard fails a manifest that declares no forbidden edge at all.
//!
//! They sit here rather than in `architecture_boundary.rs` so that file stays a
//! statement of ADR 005's rule. These are statements about the scanner.

use rstest::rstest;

use super::{ScanOutcome, parse_manifest, scan_for};

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
fn target_tables_are_read_only_at_the_dependency_depth() {
    // Cargo reads a target-specific dependency from `[target.<selector>]`
    // directly, and nowhere deeper. Verified against `cargo metadata`: for
    // `[target.'cfg(unix)'.dependencies] libc = "0.2"` it resolves `libc`, and
    // for each of `[target.'cfg(unix)'.metadata.dependencies]`,
    // `[target.'cfg(unix)'.foo.dependencies]` and `[target.a.b.dependencies]` it
    // resolves nothing at all — silently, with no warning.
    //
    // So the deep shapes are not edges, and a walk that descended on a `target.`
    // prefix would invent one. That is a false positive in the guard's own terms:
    // it would fail a manifest Cargo accepts and that declares no forbidden
    // dependency.
    let genuine = parse_manifest(concat!(
        "[target.'cfg(unix)'.dependencies]\n",
        "libc = \"0.2\"\n",
    ));
    // The reported path is built from the *parsed* key, so a quoted selector
    // renders without its quotes: `'cfg(unix)'` in source, `cfg(unix)` here.
    assert_eq!(
        scan_for(&genuine, "libc", None),
        ScanOutcome::Found("target.cfg(unix).dependencies.libc".to_owned()),
        "a target-specific dependency is a real edge and must be read"
    );

    for (label, manifest) in [
        (
            "target.<selector>.metadata",
            "[target.'cfg(unix)'.metadata.dependencies]\nfluent-templates = \"0.15\"\n",
        ),
        (
            "target.<selector>.<other>",
            "[target.'cfg(unix)'.foo.dependencies]\nfluent-templates = \"0.15\"\n",
        ),
        (
            "target.<selector>.<selector>",
            "[target.a.b.dependencies]\nfluent-templates = \"0.15\"\n",
        ),
    ] {
        assert_eq!(
            scan_for(&parse_manifest(manifest), "fluent-templates", None),
            ScanOutcome::Absent,
            "a dependency table below the selector depth is not an edge ({label})"
        );
    }

    // And the genuine table in the same manifest is still read, so this is a
    // limit on how deep the walk goes rather than a disabled target scan.
    let mixed = parse_manifest(concat!(
        "[target.'cfg(unix)'.dependencies]\n",
        "serde = \"1\"\n",
        "\n",
        "[target.'cfg(unix)'.metadata.dependencies]\n",
        "fluent-templates = \"0.15\"\n",
    ));
    assert!(
        scan_for(&mixed, "serde", None).is_found(),
        "the genuine target table must still be scanned"
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
    // Without this case the workspace clause of the walk is unreachable: every
    // other test passes the workspace table as the *resolution* argument to
    // `scan_for`, never as the document under scan. Dropping the clause left
    // all 105 tests passing, which is what makes this case load-bearing rather
    // than decorative.
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
