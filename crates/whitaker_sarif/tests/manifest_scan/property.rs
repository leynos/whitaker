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
//! `toml::Table` iterates in key order, which makes an adversarial ordering
//! reproducible but also easy to mistake for the rule itself. The generator
//! therefore emits keys that sort on both sides of the declaring keys.
//!
//! It varies *where* an entry is written, over the six positions Cargo reads:
//! the three table names at the top level, and the same three inside a
//! `[target.<selector>]` body. It also varies inheritance — the one shape a
//! member manifest cannot resolve in isolation — producing entries whose
//! workspace declaration renames the package, names another, or is absent. The
//! workspace table is a separate document, as in the repository.
//!
//! Three properties are asserted, and they are not three restatements of one:
//! a declaration is always found; no declaration plus an unreadable entry is
//! exactly `Unresolved` (not merely "not absent", which a scan that gave up
//! would also satisfy); and a fully readable manifest naming nothing else is
//! `Absent` — the only case that may be.

use proptest::prelude::*;

use super::{ScanOutcome, scan_for};

/// The package the generated manifests declare, when they declare one.
const PACKAGE: &str = "fluent-templates";

/// A package no generated entry ever declares.
///
/// Every readable non-declaring inherited entry resolves to this, so a property
/// can generate a readable inherited entry without accidentally generating a
/// second declaration.
const OTHER_PACKAGE: &str = "serde";

/// The three dependency table names Cargo reads at any one scope.
const TABLE_NAMES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// The target selector the target-scoped positions write under.
///
/// One literal selector rather than an arbitrary one: the scan iterates the
/// `target` table's keys, so a single well-formed selector exercises that
/// iteration without also testing TOML's key quoting.
const TARGET_SELECTOR: &str = "cfg(unix)";

/// The key a generated local declaration is written under.
const DECLARING_KEY: &str = "mmm_declares";

/// The key a generated inherited rename is written under.
///
/// Distinct from [`DECLARING_KEY`] because both must be able to appear in one
/// document: a second declaring entry is legal TOML only under its own key.
const RENAMED_KEY: &str = "mmm_renamed";

/// The key of an inherited entry the workspace resolves to [`OTHER_PACKAGE`].
const INHERITED_OTHER_KEY: &str = "aaa";

/// The key of a plain local entry naming nothing the scan looks for.
///
/// Distinct from [`INHERITED_OTHER_KEY`] so the two can share a table without
/// colliding. It sorts before the declaring keys, so a scan that returned on its
/// first non-matching entry would still have to reach a later declaration.
const OTHER_KEY: &str = "aab";

/// The key of an inherited entry the workspace does not declare at all.
///
/// This is what makes the fail-closed property testable. It sorts after every
/// declaring key, so an unreadable entry cannot short-circuit past one.
const UNREADABLE_KEY: &str = "zzz";

/// One of the six dependency table positions a member manifest may use.
///
/// The six are Cargo's whole contribution from a member manifest: the three
/// table names at the top level, and the same three under one target selector.
/// A `dependencies` key anywhere else is not an edge, which is why this is an
/// enumeration rather than a free path string.
#[derive(Debug, Clone, Copy)]
enum TablePath {
    /// One of [`TABLE_NAMES`] directly on the manifest.
    TopLevel(&'static str),
    /// One of [`TABLE_NAMES`] inside `[target.'cfg(unix)']`.
    Targeted(&'static str),
}

/// Generates one of the six table positions.
fn arbitrary_table_path() -> impl Strategy<Value = TablePath> {
    prop_oneof![
        Just(TablePath::TopLevel(TABLE_NAMES[0])),
        Just(TablePath::TopLevel(TABLE_NAMES[1])),
        Just(TablePath::TopLevel(TABLE_NAMES[2])),
        Just(TablePath::Targeted(TABLE_NAMES[0])),
        Just(TablePath::Targeted(TABLE_NAMES[1])),
        Just(TablePath::Targeted(TABLE_NAMES[2])),
    ]
}

/// Returns a stable tag for a [`TablePath`], for deduplication only.
///
/// `TablePath` does not derive `Ord`: it is a generator input, and ordering it
/// would invite code to sort by it. A tag keeps the deduplication key `Ord`
/// without making that an advertised property of the type.
const fn table_tag(path: TablePath) -> (bool, &'static str) {
    match path {
        TablePath::TopLevel(name) => (false, name),
        TablePath::Targeted(name) => (true, name),
    }
}

/// The entry shapes the generator can produce.
///
/// Each variant fixes its own key, because the key and the shape are not
/// independent: an inherited entry's verdict comes from the workspace's
/// declaration *for that key*, so a shape placed under an arbitrary key would
/// produce a document whose meaning the generator did not intend.
#[derive(Debug, Clone, Copy)]
enum Entry {
    /// A local declaration — `mmm_declares = { package = PACKAGE }`.
    DeclaresLocally,
    /// An inherited declaration whose workspace entry renames the package.
    InheritsRename,
    /// A plain entry naming some other package — `other = "1"`.
    Other,
    /// An inherited entry whose workspace entry names another package.
    InheritsOther,
    /// An inherited entry the workspace does not declare at all.
    Unreadable,
}

/// Generates one entry shape that does *not* name the package.
///
/// The three variants here are the whole non-declaring space: readable and
/// naming something else, readable and inherited, and unreadable. A property
/// that generated a declaring shape among these would be asserting the wrong
/// verdict.
fn arbitrary_non_declaring() -> impl Strategy<Value = Entry> {
    prop_oneof![
        Just(Entry::Other),
        Just(Entry::InheritsOther),
        Just(Entry::Unreadable),
    ]
}

/// Returns the key `entry` is written under.
const fn entry_key(entry: Entry) -> &'static str {
    match entry {
        Entry::DeclaresLocally => DECLARING_KEY,
        Entry::InheritsRename => RENAMED_KEY,
        Entry::InheritsOther => INHERITED_OTHER_KEY,
        Entry::Unreadable => UNREADABLE_KEY,
        Entry::Other => OTHER_KEY,
    }
}

/// Returns whether `entry` names the package under scan.
///
/// The generator's own oracle, used to assert that a generated document
/// contains no declaration when a property needs that. Keeping it beside the
/// generator makes the coupling explicit: if a variant is added, this must be
/// updated or the properties above it stop meaning what they say.
const fn names_package(entry: Entry) -> bool {
    matches!(entry, Entry::DeclaresLocally | Entry::InheritsRename)
}

/// Builds a table naming `package` under its `package` key.
///
/// A local declaration and a workspace declaration have the same shape; only
/// which document holds them differs.
fn declaration_of(package: &str) -> toml::Value {
    let mut declaration = toml::Table::new();
    let _previous = declaration.insert(
        "package".to_owned(),
        toml::Value::String(package.to_owned()),
    );
    toml::Value::Table(declaration)
}

/// Builds one dependency entry's TOML value from its shape.
fn entry_value(entry: Entry) -> toml::Value {
    match entry {
        Entry::DeclaresLocally => declaration_of(PACKAGE),
        Entry::InheritsRename | Entry::InheritsOther | Entry::Unreadable => {
            let mut value = toml::Table::new();
            let _previous = value.insert("workspace".to_owned(), toml::Value::Boolean(true));
            toml::Value::Table(value)
        }
        Entry::Other => toml::Value::String("1".to_owned()),
    }
}

/// One generated entry: the table position it occupies and its shape.
type Placed = (TablePath, Entry);

/// Drops any entry already occupying `path` under `key`.
///
/// A generated list may place a plain entry where a forced one must go. Left in
/// place, the forced entry would be the duplicate and deduplication, which keeps
/// the *first* occurrence, would silently discard it — leaving a document that
/// does not contain the shape the property needs.
fn remove_same_slot(entries: &mut Vec<Placed>, path: TablePath, key: &str) {
    entries.retain(|(other_path, entry)| {
        !(table_tag(*other_path) == table_tag(path) && entry_key(*entry) == key)
    });
}

/// Keeps the first occurrence of each `(position, key)` pair, so a manifest is
/// valid TOML.
///
/// A `toml::Table` cannot hold a repeated key within one table, so a duplicate
/// would fail to parse before the scan saw it. Deduplicating on the position
/// and key alone — not on the whole entry — is the point: the same key may
/// legally appear in two different positions, and two entries may share a shape
/// and must still get distinct keys.
fn deduplicated(entries: Vec<Placed>) -> Vec<Placed> {
    let mut seen = std::collections::BTreeSet::new();
    entries
        .into_iter()
        .filter(|(path, entry)| seen.insert((table_tag(*path), entry_key(*entry))))
        .collect()
}

/// Returns the table a `path` names inside `document`, creating it.
fn table_mut(document: &mut toml::Table, path: TablePath) -> &mut toml::Table {
    match path {
        TablePath::TopLevel(name) => child_mut(document, name),
        TablePath::Targeted(name) => {
            let selector = child_mut(child_mut(document, "target"), TARGET_SELECTOR);
            child_mut(selector, name)
        }
    }
}

/// Returns the sub-table `name` of `parent`, creating an empty one if absent.
///
/// # Panics
///
/// Panics when `name` is already present as a non-table value, which would mean
/// the generator produced a document no manifest could contain.
fn child_mut<'a>(parent: &'a mut toml::Table, name: &str) -> &'a mut toml::Table {
    let value = parent
        .entry(name.to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    // Let-else rather than a `match` yielding the table: the workspace denies
    // `option_if_let_else`, whose suggested `map_or_else` would put a panic in a
    // combinator. The sibling `discovery_tests` module meets the same trio.
    let Some(table) = value.as_table_mut() else {
        panic!("{name} must be a table");
    };
    table
}

/// Builds a member manifest from generated entries.
fn document_of(entries: &[Placed]) -> toml::Table {
    let mut document = toml::Table::new();
    for (path, entry) in entries {
        let _previous = table_mut(&mut document, *path)
            .insert(entry_key(*entry).to_owned(), entry_value(*entry));
    }
    document
}

/// Builds the workspace table a generated member manifest resolves against.
///
/// Declares the rename target and the other-package keys, and deliberately
/// omits [`UNREADABLE_KEY`] so an entry inheriting through it stays unresolved.
fn workspace_table() -> toml::Table {
    let mut declarations = toml::Table::new();
    for (key, named) in [
        (RENAMED_KEY, PACKAGE),
        (INHERITED_OTHER_KEY, OTHER_PACKAGE),
        (OTHER_KEY, OTHER_PACKAGE),
    ] {
        let declaration = declaration_of(named);
        let _previous = declarations.insert(key.to_owned(), declaration);
    }
    declarations
}

/// Generates a document that certainly declares `PACKAGE`.
///
/// The declaring entry is inserted at a generated position in a generated table
/// position, so it lands before, between, and after the other entries and
/// arrives through the top level or a target selector, locally or by
/// inheritance. It is deduplicated last, which is what guarantees the
/// declaration survives: the declaring keys share no position-and-key pair with
/// any generated non-declaring entry.
fn document_with_a_declaration() -> impl Strategy<Value = toml::Table> {
    (
        prop::collection::vec((arbitrary_table_path(), arbitrary_non_declaring()), 0..3),
        any::<prop::sample::Index>(),
        arbitrary_table_path(),
        prop_oneof![Just(Entry::DeclaresLocally), Just(Entry::InheritsRename)],
    )
        .prop_map(|(mut others, position, declaring_path, declaring)| {
            // The declaring entry must not share a position with a
            // non-declaring entry of the same key, or deduplication would drop
            // it. Filtering first is cheaper and clearer than relying on the
            // index landing elsewhere.
            remove_same_slot(&mut others, declaring_path, entry_key(declaring));
            let at = position.index(others.len() + 1);
            others.insert(at, (declaring_path, declaring));
            let entries = deduplicated(others);
            debug_assert!(
                entries.iter().any(|(_, entry)| names_package(*entry)),
                "the declaration must survive deduplication"
            );
            document_of(&entries)
        })
}

/// Generates entries with no declaration of `PACKAGE`, one of which is
/// unreadable.
///
/// The forced unreadable entry is what makes the fail-closed rule testable:
/// without it the scan could legitimately report `Absent`.
fn document_with_unreadable_and_no_declaration() -> impl Strategy<Value = toml::Table> {
    (
        prop::collection::vec((arbitrary_table_path(), arbitrary_non_declaring()), 0..3),
        arbitrary_table_path(),
    )
        .prop_map(|(mut others, unreadable_path)| {
            let unreadable = (unreadable_path, Entry::Unreadable);
            remove_same_slot(&mut others, unreadable_path, entry_key(Entry::Unreadable));
            others.push(unreadable);
            document_of(&deduplicated(others))
        })
}

/// Generates entries where every entry is readable and none names `PACKAGE`.
fn document_without_the_package() -> impl Strategy<Value = toml::Table> {
    prop::collection::vec(
        (
            arbitrary_table_path(),
            prop_oneof![Just(Entry::Other), Just(Entry::InheritsOther)],
        ),
        1..4,
    )
    .prop_map(|entries| document_of(&deduplicated(entries)))
}

proptest! {
    /// A declaration is found wherever it sits and whatever surrounds it.
    ///
    /// This is the precedence rule in its positive direction. The declaration
    /// arrives through a generated table position, locally or by inheritance,
    /// so target selectors and the workspace resolution path participate rather
    /// than only the top-level table. The number of unreadable entries varies
    /// from none to three, and their keys sort on both sides of the declaring
    /// keys, so a scan that let an unreadable entry short-circuit would fail
    /// this property within a few cases.
    #[test]
    fn a_declaration_is_found_however_it_is_surrounded(document in document_with_a_declaration()) {
        let workspace = workspace_table();
        let outcome = scan_for(&document, PACKAGE, Some(&workspace));
        prop_assert!(
            outcome.is_found(),
            "a declaration must be found, got {:?} from {document:?}",
            outcome
        );
    }

    /// An unreadable entry with no declaration is `Unresolved`, not `Absent`.
    ///
    /// The exact verdict matters: asserting only "not absent" would accept a
    /// scan that returned nothing at all, which is the failure mode the
    /// fail-closed rule exists to prevent.
    #[test]
    fn an_unreadable_entry_with_no_declaration_is_unresolved(
        document in document_with_unreadable_and_no_declaration(),
    ) {
        let workspace = workspace_table();
        let outcome = scan_for(&document, PACKAGE, Some(&workspace));
        prop_assert!(
            matches!(outcome, ScanOutcome::Unresolved(_)),
            "an unreadable entry with no declaration must be unresolved, got {:?} from {document:?}",
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
        document in document_without_the_package(),
    ) {
        let workspace = workspace_table();
        let outcome = scan_for(&document, PACKAGE, Some(&workspace));
        prop_assert_eq!(outcome, ScanOutcome::Absent);
    }
}
