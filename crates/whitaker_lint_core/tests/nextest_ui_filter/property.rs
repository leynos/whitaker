//! Property tests for the nextest filter evaluator and override resolution.
//!
//! The concrete guards in the parent module pin the six identities that the
//! repository's `.config/nextest.toml` must keep selecting. They cannot say
//! anything about the *evaluator*: precedences, negation, and the two different
//! default matchers are only exercised by the handful of clauses those six
//! identities happen to reach. A regression that inverted a precedence, or that
//! made `package` a substring match, would leave every concrete case passing as
//! long as the checked-in clauses still selected the checked-in tests.
//!
//! These properties generate filter expressions as a syntax tree, render each
//! one, and compare two independent evaluations: the crate's text evaluator in
//! [`FilterExpression::selects`], and a reference interpreter that walks the
//! tree directly and never parses. Agreement between the two is the invariant;
//! disagreement names the rendered clause, so a failure reports the construct
//! that broke rather than a line number.
//!
//! Every generated clause is built *around* a concrete identity, so its leaves
//! are a mix of true and false. That is load-bearing rather than cosmetic: if
//! every leaf were false, `a & b`, `a | b`, and their transposition would all
//! agree, and the comparison would pass while the precedence it exists to
//! check was inverted.
//!
//! The generated alphabet deliberately includes non-ASCII letters. The
//! evaluator scans `char_indices` and slices with `get`, and a filter argument
//! containing a multi-byte character is exactly the input that would turn a
//! byte-offset mistake into a panic inside the guard that exists to protect the
//! build.
//!
//! Override resolution has its own property: nextest applies each setting from
//! the first override that matches *and* configures it, so the ordering of a
//! generated list, not merely the presence of a matching entry, decides the
//! governing override.

use proptest::prelude::*;
use toml::Value;

use super::{
    config::governing_override,
    filter_model::{BinaryName, FilterExpression, PackageName, TestIdentity, TestName},
};

/// The most overrides a generated profile may declare.
///
/// Larger lists buy nothing: the property is about which entry wins, and that
/// is decided within the first few positions of any list.
const MAX_OVERRIDES: usize = 6;

/// The deepest nesting a generated filter tree may reach.
///
/// `proptest` shrinks towards the shallow end, so the bound exists to reach the
/// interesting compound cases at all rather than to keep them out.
const MAX_TREE_DEPTH: u32 = 3;

/// A filter expression as a syntax tree, independent of any text form.
///
/// The tree is what the reference interpreter consumes. It exists so the
/// comparison is against a genuinely second implementation of the grammar
/// rather than against the same string parser run twice.
#[derive(Clone, Debug)]
enum Node {
    /// `package(name)`, which nextest matches exactly by default.
    Package(String),
    /// `binary(name)`, which nextest matches exactly by default.
    Binary(String),
    /// `test(fragment)`, which nextest matches as a substring by default.
    Test(String),
    /// `not inner`.
    Not(Box<Self>),
    /// `inner & inner & ...`.
    All(Vec<Self>),
    /// `inner | inner | ...`.
    AnyOf(Vec<Self>),
}

/// An owned test identity, so a generated identity can outlive the closure that
/// built it. [`TestIdentity`] borrows its three names.
#[derive(Clone, Debug)]
struct OwnedIdentity {
    /// The package that owns the test.
    package: String,
    /// The test binary nextest runs it from.
    binary: String,
    /// The name nextest reports for it.
    name: String,
}

impl OwnedIdentity {
    /// Borrows this identity for the evaluator.
    fn as_identity(&self) -> TestIdentity<'_> {
        TestIdentity {
            package: PackageName(&self.package),
            binary: BinaryName(&self.binary),
            name: TestName(&self.name),
        }
    }
}

/// Characters a generated name may contain.
///
/// The set excludes every character the filter grammar gives a meaning — `&`,
/// `|`, `(`, `)`, and the `=` exact-match prefix — so a generated name can only
/// ever be an argument, never a clause. It also excludes the glob
/// metacharacters and matcher prefixes that the model deliberately reports as
/// errors instead of evaluating. What remains is ordinary identifier text plus
/// a non-ASCII range.
fn name_character() -> impl Strategy<Value = char> {
    prop_oneof![
        prop::char::range('a', 'z'),
        prop::char::range('A', 'Z'),
        prop::char::range('0', '9'),
        Just('_'),
        prop::char::range('\u{3b1}', '\u{3c9}'),
    ]
}

/// A non-empty name drawn from [`name_character`].
fn name() -> impl Strategy<Value = String> {
    prop::collection::vec(name_character(), 1..=8)
        .prop_map(|characters| characters.into_iter().collect())
}

/// A name that is either the anchored text or a fresh one.
///
/// The anchored branch is what gives each generated clause a true leaf; see the
/// module comment for why that is required. The anchor is taken by value because
/// a strategy outlives the closure that builds it, so it cannot borrow from the
/// identity it was derived from.
fn anchored_name(anchor: String) -> impl Strategy<Value = String> {
    prop_oneof![2 => Just(anchor), 1 => name()]
}

/// Generates an identity whose package, binary, and test names may share text.
///
/// Sharing is the point. A substring-matching `package` clause and an
/// exact-matching one only differ when the generated name is a fragment of
/// another generated name, and the override guards exist precisely because a
/// shared name was once selected by the wrong clause.
fn identity() -> impl Strategy<Value = OwnedIdentity> {
    name()
        .prop_flat_map(|package| {
            let shared_binary = package.clone();
            let shared_name = package.clone();
            (
                Just(package),
                prop_oneof![3 => Just(shared_binary), 1 => name()],
                prop_oneof![
                    1 => Just(shared_name),
                    3 => name(),
                    2 => name().prop_map(|suffix| format!("ui::{suffix}")),
                ],
            )
        })
        .prop_map(|(package, binary, test_name)| OwnedIdentity {
            package,
            binary,
            name: test_name,
        })
}

/// Generates an identity drawn from `anchor`: each field is either the anchor's
/// own text or a fresh name.
fn related_identity(anchor: OwnedIdentity) -> impl Strategy<Value = OwnedIdentity> {
    (
        anchored_name(anchor.package),
        anchored_name(anchor.binary),
        anchored_name(anchor.name),
    )
        .prop_map(|(package, binary, test_name)| OwnedIdentity {
            package,
            binary,
            name: test_name,
        })
}

/// A leaf clause anchored to `identity`, or (recursively) a compound one.
///
/// The identity is taken by value because `impl Strategy` captures every
/// in-scope lifetime in edition 2024, so a borrowing parameter would make the
/// strategy borrow from the closure that returns it.
fn node_for(identity: OwnedIdentity) -> impl Strategy<Value = Node> {
    let leaf = prop_oneof![
        anchored_name(identity.package).prop_map(Node::Package),
        anchored_name(identity.binary).prop_map(Node::Binary),
        anchored_name(identity.name).prop_map(Node::Test),
    ];

    leaf.prop_recursive(MAX_TREE_DEPTH, 4, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|held| Node::Not(Box::new(held))),
            prop::collection::vec(inner.clone(), 2..=3).prop_map(Node::All),
            prop::collection::vec(inner, 2..=3).prop_map(Node::AnyOf),
        ]
    })
}

/// Renders a clause as an operand, parenthesized when it is compound.
///
/// Parenthesizing every compound operand keeps the rendering unambiguous
/// without having to reproduce the evaluator's precedence rules — reproducing
/// them here would make the property agree with a precedence bug by
/// construction.
fn render_operand(node: &Node) -> String {
    match node {
        Node::Package(_) | Node::Binary(_) | Node::Test(_) => render(node),
        Node::Not(_) | Node::All(_) | Node::AnyOf(_) => format!("({})", render(node)),
    }
}

/// Renders a clause as nextest filter text.
fn render(node: &Node) -> String {
    match node {
        Node::Package(argument) => format!("package({argument})"),
        Node::Binary(argument) => format!("binary({argument})"),
        Node::Test(argument) => format!("test({argument})"),
        Node::Not(inner) => format!("not {}", render_operand(inner)),
        Node::All(children) => children
            .iter()
            .map(render_operand)
            .collect::<Vec<_>>()
            .join(" & "),
        Node::AnyOf(children) => children
            .iter()
            .map(render_operand)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

/// Evaluates the tree directly, without rendering or parsing.
///
/// This is the reference the text evaluator is compared against, so it must
/// stay a separate implementation: it consumes the tree, and the only thing it
/// shares with [`FilterExpression::selects`] is the meaning of the three leaf
/// operators.
fn reference_holds(node: &Node, identity: TestIdentity<'_>) -> bool {
    match node {
        Node::Package(argument) => identity.package.0 == argument.as_str(),
        Node::Binary(argument) => identity.binary.0 == argument.as_str(),
        Node::Test(fragment) => identity.name.0.contains(fragment.as_str()),
        Node::Not(inner) => !reference_holds(inner, identity),
        Node::All(children) => children.iter().all(|held| reference_holds(held, identity)),
        Node::AnyOf(children) => children.iter().any(|held| reference_holds(held, identity)),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        .. ProptestConfig::default()
    })]

    /// Rendering a generated tree and evaluating it as text agrees with the
    /// reference interpreter for every generated identity.
    ///
    /// The clauses are built from the same identity they are evaluated
    /// against, so the tree contains both true and false leaves at every level.
    #[test]
    fn rendered_expressions_agree_with_the_reference_evaluator(
        (node, identity) in identity().prop_flat_map(|identity| {
            (node_for(identity.clone()), Just(identity))
        }),
    ) {
        let rendered = render(&node);
        let actual = FilterExpression(rendered.as_str())
            .selects(identity.as_identity())
            .expect("generated expressions stay inside the modelled grammar");

        prop_assert_eq!(
            actual,
            reference_holds(&node, identity.as_identity()),
            "rendered clause {:?} disagreed for package {:?}, binary {:?}, test {:?}",
            rendered,
            identity.package,
            identity.binary,
            identity.name
        );
    }

    /// `package` and `binary` clauses select exactly the identities that carry
    /// the named package or binary, and nothing else.
    ///
    /// This is the matcher property the shadowing bug violated: a `package`
    /// clause that fell back to substring matching would still select the
    /// intended package, so only the *exclusions* catch it. Every generated
    /// list contains at least one identity built from the target, so the
    /// positive direction is not vacuous either.
    #[test]
    fn package_and_binary_clauses_select_exactly_their_own_names(
        (identities, target) in name().prop_flat_map(|target| {
            let anchor = OwnedIdentity {
                package: target.clone(),
                binary: target.clone(),
                name: target.clone(),
            };
            (
                prop::collection::vec(related_identity(anchor), 1..=6),
                Just(target),
            )
        }),
    ) {
        let package_clause = FilterExpression(&format!("package({target})"));
        let binary_clause = FilterExpression(&format!("binary({target})"));

        for owned in &identities {
            let borrowed = owned.as_identity();
            prop_assert_eq!(
                package_clause
                    .selects(borrowed)
                    .expect("a package clause stays inside the modelled grammar"),
                owned.package == target,
                "package({:?}) selected package {:?}",
                target,
                owned.package
            );
            prop_assert_eq!(
                binary_clause
                    .selects(borrowed)
                    .expect("a binary clause stays inside the modelled grammar"),
                owned.binary == target,
                "binary({:?}) selected binary {:?}",
                target,
                owned.binary
            );
        }
    }


    /// The governing override is the first entry that both matches the test and
    /// configures the setting.
    ///
    /// An override with no `filter` key matches every test, so it can govern
    /// from any position; an override whose filter does not select the test is
    /// skipped even when it configures the setting. Both directions are
    /// generated — the filters are anchored to the identity, so some of them
    /// match — because a resolver that ignored either condition would still
    /// return *some* override for the checked-in configuration.
    #[test]
    fn the_first_matching_override_governs_each_setting(
        (specs, identity) in identity().prop_flat_map(|identity| {
            let filters = node_for(identity.clone());
            let specs = prop::collection::vec(
                (any::<bool>(), prop::option::of(filters)),
                0..=MAX_OVERRIDES,
            );
            (specs, Just(identity))
        }),
    ) {
        let overrides = render_overrides(&specs);
        let expected = specs.iter().position(|(configures, filter)| {
            *configures
                && filter
                    .as_ref()
                    .is_none_or(|node| reference_holds(node, identity.as_identity()))
        });

        let actual = governing_override(&overrides, "slow-timeout", identity.as_identity())
            .map(|(index, _)| index);

        prop_assert_eq!(actual, expected);
    }
}

/// Builds the override tables the resolver is asked about.
///
/// Kept out of the property body so the generated `(configures, filter)` pairs
/// stay the single description of each table; duplicating the construction
/// inside the expectation would let the two drift.
fn render_overrides(specs: &[(bool, Option<Node>)]) -> Vec<Value> {
    specs
        .iter()
        .map(|(configures, filter)| {
            let mut table = toml::Table::new();
            if *configures {
                table.insert("slow-timeout".to_owned(), Value::String("75m".to_owned()));
            }
            if let Some(node) = filter {
                table.insert("filter".to_owned(), Value::String(render(node)));
            }
            Value::Table(table)
        })
        .collect()
}
