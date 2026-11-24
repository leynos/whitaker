//! Walking the dependency tables Cargo actually reads.
//!
//! A manifest can contain the key `dependencies` in places Cargo never
//! resolves. This module exists so the scan visits exactly the three places it
//! does — the manifest's own top level, a root manifest's
//! `[workspace.dependencies]`, and the body of a `[target.<selector>]` table —
//! and reports nothing else. A walk keyed on the table name alone would report
//! edges Cargo does not have.
//!
//! It lives apart from the scanner that consumes it because the grammar and the
//! verdict are separate concerns: this module answers *which tables exist*, and
//! `manifest_scan` decides *whether one names a forbidden package*. The
//! behavioural tests for this walk live in `table_discovery`.

use super::{DEPENDENCY_TABLES, DependencyTable};

/// A dependency table's dotted path within a manifest.
///
/// The scan reports findings as `dependencies.fluent-templates` or
/// `target.cfg(unix).dependencies.loc`, so the path is built one key at a time
/// and the empty-root case is the only subtlety: a table at the manifest's top
/// level is named by its key alone, never by a leading dot.
///
/// It is a newtype rather than a bare `String` because two `&str` parameters
/// returning a `String` is the shape that invites a caller to swap the
/// arguments. A key and a prefix are both strings and only their order is
/// correct, so the receiver distinguishes them at the type level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TablePath(String);

impl TablePath {
    /// Starts a path at the manifest's top level, with no prefix.
    pub(crate) const fn root() -> Self { Self(String::new()) }

    /// Returns this path extended by `key`.
    ///
    /// Extending a root path yields the key alone; extending any other path
    /// yields `<self>.<key>`.
    #[must_use]
    pub(crate) fn with_key(&self, key: &str) -> Self {
        if self.0.is_empty() {
            Self(key.to_owned())
        } else {
            Self(format!("{}.{key}", self.0))
        }
    }

    /// Returns the path as a string slice.
    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

/// Harvests the dependency tables `scope` may declare directly.
///
/// Every name in `DEPENDENCY_TABLES` is harvested under `prefix`, and nothing
/// deeper: a dependency table is read only where Cargo reads it, which is
/// always as a direct child of the scope this is called on.
fn harvest_under(scope: &toml::Table, prefix: &TablePath, into: &mut Vec<DependencyTable>) {
    for name in DEPENDENCY_TABLES {
        if let Some(table) = scope.get(name).and_then(toml::Value::as_table) {
            into.push((prefix.with_key(name).as_str().to_owned(), table.clone()));
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

    harvest_under(document, &TablePath::root(), &mut tables);

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
                harvest_under(
                    body,
                    &TablePath::root().with_key("target").with_key(selector),
                    &mut tables,
                );
            }
        }
    }

    tables
}
