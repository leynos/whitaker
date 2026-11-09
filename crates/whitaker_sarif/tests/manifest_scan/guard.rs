//! The end-to-end guard path a manifest travels.
//!
//! `architecture_boundary.rs` asserts ADR 005's dependency rule, and every
//! assertion it makes runs the same three steps: prove the scan is not vacuous,
//! scan for the forbidden package, and require the scan to have *certified
//! absence*. This module is the single place those steps are composed, so the
//! real manifests and the fixture manifests cannot drift onto different paths.
//!
//! Composing them in one place is not tidiness. Split the steps and a caller
//! can silently drop one: a test that calls [`scan_for`] directly never touches
//! [`assert_non_vacuous`], so the non-vacuity floor could be deleted outright
//! and every such test would still pass. The floor exists to catch a manifest
//! the scan reads but does not understand, and only a funnel that runs both can
//! show it is still wired in.
//!
//! Two entry points share that body. [`assert_no_edge`] takes a path and is
//! what a real manifest uses, including the workspace lookup. [`assert_no_edge_in`]
//! takes an already-parsed document and is what a fixture uses, because a
//! fixture models a member manifest beside a literal workspace table rather than
//! staging two files. They differ only in where the bytes come from; the rule
//! they apply is [`assert_absent`] in both cases.
//!
//! [`scan_for`]: super::scan_for
//! [`assert_non_vacuous`]: super::assert_non_vacuous

use camino::Utf8Path;

use super::{assert_non_vacuous, parse_manifest, read_manifest, scan_for, workspace_dependencies};

/// Asserts that the manifest on disk at `path` declares no `forbidden` edge.
///
/// This is ADR 005's rule applied to one manifest and one forbidden package.
/// The workspace table is resolved here rather than accepted from the caller,
/// so a caller cannot omit it and thereby blind the guard to an edge inherited
/// through `{ workspace = true }`.
///
/// A discovery failure is a guard failure rather than an `Err` return, so this
/// reports through [`assert_absent`]'s panics. An `Err` return would let a
/// caller that ignores it treat an unreadable manifest as a passing one, which
/// is the failure mode the guard exists to prevent.
///
/// # Panics
///
/// Panics when the manifest cannot be read or parsed; when it declares no
/// dependency table or no entry; when the root workspace manifest cannot be
/// interrogated; and when the scan finds the forbidden package or leaves an
/// entry unresolved.
pub(crate) fn assert_no_edge(path: &Utf8Path, crate_name: &str, forbidden: &str) {
    let document = parse_manifest(&read_manifest(path));
    let workspace = read_workspace();
    assert_absent(&document, crate_name, forbidden, workspace.as_ref());
}

/// Asserts that a fixture document declares no `forbidden` edge.
///
/// The fixture counterpart of [`assert_no_edge`], running the same rule over a
/// document the caller already has. `workspace` is the literal
/// `[workspace.dependencies]` table the fixture resolves against, or `None` for
/// a fixture that models a member whose root declares nothing — in which case
/// its inheriting entries stay unresolved, which is itself one of the shapes
/// the fixture set covers.
///
/// # Panics
///
/// Panics for the same reasons as [`assert_no_edge`], minus the ones that
/// concern reading a file.
pub(crate) fn assert_no_edge_in(
    document: &toml::Table,
    crate_name: &str,
    forbidden: &str,
    workspace: Option<&toml::Table>,
) {
    assert_absent(document, crate_name, forbidden, workspace);
}

/// The rule itself: a non-vacuous scan that certifies the package absent.
///
/// `is_absent` rather than `!is_found` is the load-bearing choice. `Unresolved`
/// means an entry's effective name could not be read, so the forbidden package
/// may be hiding behind it; treating that as a pass would report a clean
/// manifest the guard never actually examined.
fn assert_absent(
    document: &toml::Table,
    crate_name: &str,
    forbidden: &str,
    workspace: Option<&toml::Table>,
) {
    assert_non_vacuous(document, crate_name);
    let outcome = scan_for(document, forbidden, workspace);
    assert!(
        outcome.is_absent(),
        "{crate_name} must not depend on {forbidden}, but {}",
        outcome.finding()
    );
}

/// Reads the root workspace's `[workspace.dependencies]` table for the guard.
///
/// An unreadable root fails the guard rather than being read as a member with
/// no workspace. Falling back to `None` would leave every inherited entry
/// unresolved and fail the assertion for the wrong reason, reporting a root
/// problem as a forbidden edge.
///
/// # Panics
///
/// Panics when the root manifest cannot be interrogated.
fn read_workspace() -> Option<toml::Table> {
    match workspace_dependencies() {
        Ok(table) => table,
        Err(error) => panic!("the guard cannot resolve the root workspace: {error}"),
    }
}
