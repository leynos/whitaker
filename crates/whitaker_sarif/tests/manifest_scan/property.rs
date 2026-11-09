//! Property tests for the manifest scan's verdict precedence.
//!
//! `scan_for` has three outcomes and a precedence rule between them: a
//! declaration anywhere outranks an unreadable entry anywhere, and only when no
//! entry declares the package does an unreadable one decide the result. The
//! example tests elsewhere pin that rule for the shapes someone thought of. The
//! failure mode it really has to survive is combinatorial: an entry that sorts
//! ahead of the declaring one, a table visited in a different position, a
//! manifest that mixes every shape at once.
//!
//! `toml::Table` happens to iterate in key order, which makes an adversarial
//! ordering reproducible but also easy to mistake for the rule itself. The
//! properties below therefore hold for *any* generated manifest, and the
//! generator deliberately produces keys that sort on both sides of the declaring
//! key so the ordering is exercised rather than avoided.
//!
//! Three properties are asserted, and they are not three restatements of one:
//!
//! 1. **A declaration is always found.** However many unreadable entries surround it and wherever
//!    it sits, the scan reports the declaring key.
//! 2. **An unreadable entry is never absent.** Without a declaration, any unresolved entry makes
//!    the verdict `Unresolved` rather than `Absent`.
//! 3. **Neither is a clean manifest.** When every entry is readable and none names the package, the
//!    scan reports `Absent` — the only case that may.

use proptest::prelude::*;

use super::{ScanOutcome, parse_manifest, scan_for, workspace_table_of};

/// The package the generated manifests declare, when they declare one.
const PACKAGE: &str = "fluent-templates";

/// The workspace table the generated manifests resolve against.
///
/// It deliberately declares none of the generated keys, so every
/// `{ workspace = true }` entry in a generated manifest is unreadable. That is
/// what makes the fail-closed property testable: if the table declared them,
/// they would resolve and there would be nothing to fail closed about.
const WORKSPACE_WITHOUT_THEM: &str = concat!("[workspace.dependencies]\n", "serde = \"1\"\n");

/// The entry shapes a generated manifest can carry.
///
/// The three shapes are the ones `names_package` distinguishes: a readable
/// entry that names the package, a readable one that names something else, and
/// an inherited one whose name the workspace table cannot supply.
#[derive(Debug, Clone, Copy)]
enum Shape {
    /// A plain key naming the package — `fluent-templates = "0.15"`.
    Declares,
    /// A plain key naming a package nobody scanned — `other = "1"`.
    Other,
    /// An inherited entry the given workspace cannot resolve.
    Unresolved,
}

/// Generates one entry shape.
fn arbitrary_shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        Just(Shape::Declares),
        Just(Shape::Other),
        Just(Shape::Unresolved),
    ]
}

/// Generates the keys a manifest's entries are written under.
///
/// Keys come from disjoint prefixes — `aaa`, `mmm`, `zzz` — and a manifest
/// draws each key at most once, so the generated TOML can never repeat a key.
/// That is a generator obligation, not a stylistic one: a repeated key makes
/// the manifest unparsable, and a property that generates one is testing the
/// TOML parser rather than the scan.
///
/// The prefixes are chosen so a manifest mixes keys that sort before, at, and
/// after the declaring key. `toml::Table` iterates in key order, so a scan that
/// returned on its first unresolved entry would fail about half the generated
/// cases rather than always passing or always failing.
fn arbitrary_other_key() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("aaa".to_owned()),
        Just("aab".to_owned()),
        Just("zzz".to_owned()),
        Just("zzz_n".to_owned()),
    ]
}

/// The key the generated manifest writes its declaration under.
const DECLARING_KEY: &str = "mmm_declares";

/// The key a forced unreadable entry is written under.
const UNREADABLE_KEY: &str = "qqq_unreadable";

/// Keeps the first occurrence of each key, so a rendered manifest is valid TOML.
///
/// A `toml::Table` cannot hold a repeated key, so a duplicate would fail to
/// parse before the scan saw it. Deduplicating on the key alone — not on the
/// whole entry — is the point: two entries may share a shape and must still get
/// distinct keys.
fn deduplicated(entries: Vec<(String, Shape)>) -> Vec<(String, Shape)> {
    let mut seen = std::collections::BTreeSet::new();
    entries
        .into_iter()
        .filter(|(key, _)| seen.insert(key.clone()))
        .collect()
}

/// Builds a `[dependencies]` table from `(key, shape)` entries.
///
/// The table is assembled directly rather than rendered to TOML and reparsed.
/// That is not a shortcut around the parser: the scan's subject is a parsed
/// `toml::Table`, so the parse step is shared setup rather than anything the
/// property is about, and building the value directly removes a failure mode
/// the generator would otherwise own — a repeated key makes TOML invalid, and a
/// property that tripped over one would be testing the parser instead of the
/// scan. Deduplication upstream keeps the keys distinct for the same reason.
fn dependency_table(entries: &[(String, Shape)]) -> toml::Table {
    let mut table = toml::Table::new();
    for (key, shape) in entries {
        let value = match shape {
            Shape::Declares => {
                let mut entry = toml::Table::new();
                let _previous = entry.insert(
                    "package".to_owned(),
                    toml::Value::String(PACKAGE.to_owned()),
                );
                toml::Value::Table(entry)
            }
            Shape::Other => toml::Value::String("1".to_owned()),
            Shape::Unresolved => {
                let mut entry = toml::Table::new();
                let _previous = entry.insert("workspace".to_owned(), toml::Value::Boolean(true));
                toml::Value::Table(entry)
            }
        };
        let _previous = table.insert(key.clone(), value);
    }
    table
}

/// Wraps a dependency table in a document that declares it.
///
/// The table is filed under `dependencies`, which is where Cargo reads one, so
/// the scan sees exactly the entries the generator produced and nothing else.
fn document_of(entries: &[(String, Shape)]) -> toml::Table {
    let mut document = toml::Table::new();
    let _previous = document.insert(
        "dependencies".to_owned(),
        toml::Value::Table(dependency_table(entries)),
    );
    document
}

/// Generates a document known to declare `PACKAGE`.
///
/// The declaring entry is inserted at a generated position, so it lands before,
/// between, and after the other entries across cases. It is always deduplicated
/// last, which is what guarantees the declaration survives: no generated key
/// shares its prefix, and the forced key is distinct from every other one.
fn document_with_a_declaration() -> impl Strategy<Value = toml::Table> {
    (
        prop::collection::vec((arbitrary_other_key(), arbitrary_shape()), 0..4),
        any::<prop::sample::Index>(),
    )
        .prop_map(|(mut others, position)| {
            let at = position.index(others.len() + 1);
            others.insert(at, (DECLARING_KEY.to_owned(), Shape::Declares));
            let entries = deduplicated(others);
            debug_assert!(
                entries.iter().any(|(key, _)| key == DECLARING_KEY),
                "the declaration must survive deduplication"
            );
            document_of(&entries)
        })
}

proptest! {
    /// A declaration is found wherever it sits and whatever surrounds it.
    ///
    /// This is the precedence rule in its positive direction. The number of
    /// unreadable entries varies from none to three, and their keys sort on
    /// both sides of the declaring key, so a scan that let an unreadable entry
    /// short-circuit would fail this property within a few cases.
    #[test]
    fn a_declaration_is_found_however_it_is_surrounded(document in document_with_a_declaration()) {
        let workspace = parse_manifest(WORKSPACE_WITHOUT_THEM);
        let resolved = workspace_table_of(&workspace);

        let outcome = scan_for(&document, PACKAGE, resolved.as_ref());
        prop_assert!(
            outcome.is_found(),
            "a declaration must be found, got {:?} from {document:?}",
            outcome
        );
    }

    /// An unreadable entry is never certified absent.
    ///
    /// This is the fail-closed rule in its general form: with no declaration
    /// present, the scan may report `Unresolved` or `Absent`, but `Absent` only
    /// when there was nothing unreadable to fail on. The generator forces at
    /// least one unreadable entry, so `Absent` is always wrong here.
    #[test]
    fn an_unreadable_entry_is_never_absent(
        others in prop::collection::vec((arbitrary_other_key(), arbitrary_shape()), 0..4),
    ) {
        let mut entries = others;
        entries.push((UNREADABLE_KEY.to_owned(), Shape::Unresolved));
        let document = document_of(&deduplicated(entries));
        let workspace = parse_manifest(WORKSPACE_WITHOUT_THEM);
        let resolved = workspace_table_of(&workspace);

        let outcome = scan_for(&document, PACKAGE, resolved.as_ref());
        prop_assert!(
            !outcome.is_absent(),
            "an unreadable entry must not certify absence, got {:?} from {document:?}",
            outcome
        );
    }

    /// A fully readable manifest that names nothing else is `Absent`.
    ///
    /// The third outcome needs a property too, or the two above could be
    /// satisfied by a scan that never reported `Absent` at all — which would
    /// fail every clean manifest in the guard.
    #[test]
    fn a_readable_manifest_without_the_package_is_absent(
        keys in prop::collection::vec(arbitrary_other_key(), 1..4),
    ) {
        let entries: Vec<(String, Shape)> =
            keys.into_iter().map(|key| (key, Shape::Other)).collect();
        let document = document_of(&deduplicated(entries));

        let outcome = scan_for(&document, PACKAGE, None);
        prop_assert_eq!(outcome, ScanOutcome::Absent);
    }
}
