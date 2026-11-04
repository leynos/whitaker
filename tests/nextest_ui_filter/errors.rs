//! Error-surface tests for the nextest filter model.
//!
//! The evaluator models only the grammar `.config/nextest.toml` uses. Every
//! construct outside it is a [`FilterError`] rather than a quiet `false`, and
//! these cases pin that boundary in both directions: a clause the model does
//! not implement must be reported, and a clause it *does* implement must not be
//! reported as unsupported by accident.
//!
//! The distinction matters more than it looks. A guard that answered "does not
//! select" for a malformed clause is indistinguishable from a correct negative,
//! so a filter that grew a new operator would silently stop selecting the
//! harnesses it was written for while every assertion still passed.

use rstest::rstest;
use toml::Value;

use super::{
    config::{filter_of, governing_override},
    filter_model::{
        BinaryName,
        FilterError,
        FilterExpression,
        PackageName,
        TestIdentity,
        TestName,
    },
};

/// An identity that any well-formed clause in these cases can be evaluated
/// against.
const fn sample_identity() -> TestIdentity<'static> {
    TestIdentity {
        package: PackageName("no_unwrap_or_else_panic"),
        binary: BinaryName("no_unwrap_or_else_panic"),
        name: TestName("ui::example_compiles_under_test_harness"),
    }
}

/// Parses a TOML document that these cases expect to be well formed.
///
/// The fixtures are written by hand and exist to exercise the `filter` reader,
/// not the TOML parser, so a syntax error here is a defect in the fixture.
///
/// # Panics
///
/// Panics when `document` is not valid TOML.
fn table_from(document: &str) -> Value {
    document.parse::<toml::Table>().map_or_else(
        |error| panic!("the fixture should be valid TOML: {error}"),
        Value::Table,
    )
}

/// A term with no parentheses is not a clause at all.
#[test]
fn a_bare_word_is_a_malformed_term() {
    let error = FilterExpression("package")
        .selects(sample_identity())
        .expect_err("a bare word is not a nextest filter clause");

    assert_eq!(
        error,
        FilterError::MalformedTerm {
            term: "package".to_owned(),
            problem: "should be of the form `operator(argument)`",
        },
        "a term without parentheses should report its shape, not select nothing"
    );
}

/// An unterminated clause means the filter text is truncated or corrupted.
#[rstest]
#[case::missing_close("package(foo")]
#[case::missing_close_after_operator("test(")]
fn an_unterminated_clause_is_a_malformed_term(#[case] expression: &str) {
    let error = FilterExpression(expression)
        .selects(sample_identity())
        .expect_err("an unterminated clause is not a nextest filter clause");

    let FilterError::MalformedTerm { term, problem } = &error else {
        panic!("expected a malformed-term error for {expression:?}; got {error}");
    };
    assert_eq!(
        term, expression,
        "the error should quote the clause it could not read"
    );
    assert_eq!(
        *problem, "should end with a closing parenthesis",
        "an unterminated clause should report the missing parenthesis"
    );
}

/// A closing parenthesis with no opener is equally malformed.
#[test]
fn a_stray_closing_parenthesis_is_a_malformed_term() {
    let error = FilterExpression("package)")
        .selects(sample_identity())
        .expect_err("a stray closing parenthesis is not a nextest filter clause");

    assert!(
        matches!(error, FilterError::MalformedTerm { .. }),
        "a stray closing parenthesis should be reported as malformed; got {error}"
    );
}

/// nextest has more operators than this model implements. Reaching one must
/// stop the guard, because a clause it cannot read is exactly the drift the
/// guard exists to detect.
#[rstest]
#[case::default_filter("default-filter(foo)")]
#[case::kind("kind(lib)")]
#[case::platform("platform(linux)")]
fn an_unmodelled_operator_is_reported(#[case] expression: &str) {
    let error = FilterExpression(expression)
        .selects(sample_identity())
        .expect_err("an unmodelled operator must not be answered with `false`");

    let expected = expression
        .split_once('(')
        .map(|(operator, _)| operator.to_owned())
        .unwrap_or_default();
    assert_eq!(
        error,
        FilterError::UnsupportedOperator {
            operator: expected.clone(),
        },
        "the error should name the operator it does not implement"
    );
    assert!(
        error.to_string().contains(&expected),
        "the rendered error should name {expected:?}; got {error}"
    );
}

/// Glob expansion is deliberately unimplemented for `package` and `binary`, so
/// every form that would need it is an error rather than a silent `false`.
#[rstest]
#[case::star("package(foo*)")]
#[case::question("binary(bar?)")]
#[case::character_class("package(foo[ab])")]
#[case::brace("package(foo{a,b})")]
#[case::regex_prefix("package(~foo)")]
#[case::contains_prefix("package(/foo)")]
#[case::hash_prefix("package(#foo)")]
fn a_glob_argument_is_reported_as_unsupported(#[case] expression: &str) {
    let error = FilterExpression(expression)
        .selects(sample_identity())
        .expect_err("an unexpandable glob must not be answered with `false`");

    let FilterError::UnsupportedGlob { argument } = &error else {
        panic!("expected an unsupported-glob error for {expression:?}; got {error}");
    };
    assert!(
        expression.contains(argument.as_str()),
        "the error should quote the argument {argument:?} from {expression:?}"
    );
}

/// An exact-match prefix is part of the modelled grammar, so it must *not* be
/// reported as an unsupported glob.
#[test]
fn an_exact_match_prefix_stays_inside_the_grammar() {
    let selected = FilterExpression("package(=no_unwrap_or_else_panic)")
        .selects(sample_identity())
        .expect("`=` is an explicit matcher, not an unexpandable glob");

    assert!(
        selected,
        "an exact-match prefix should select the named package"
    );
}

/// A `filter` key that is absent means "matches every test".
#[test]
fn an_absent_filter_is_no_filter() {
    let table: Value = "test-group = \"serial-dylint-ui\""
        .parse::<toml::Table>()
        .map(Value::Table)
        .expect("the fixture should be valid TOML");

    assert!(
        filter_of(&table).is_none(),
        "an override without a `filter` key matches every test"
    );
}

/// A `filter` key that is a string is the clause it spells.
#[test]
fn a_string_filter_is_parsed_as_its_clause() {
    let table: Value = table_from("filter = \"package(alpha)\"");

    let filter = filter_of(&table).expect("a string filter should parse");
    assert_eq!(
        filter.0, "package(alpha)",
        "the parsed filter should keep the clause text"
    );
}

/// A non-string `filter` is a configuration error, not an absent filter.
///
/// Mapping it to `None` would make the override match every test
/// unconditionally, which is the opposite of what a filter that failed to parse
/// should do — and it would silently widen the override to the whole workspace.
#[rstest]
#[case::integer("filter = 42")]
#[case::boolean("filter = true")]
#[case::array("filter = [\"package(alpha)\"]")]
#[should_panic(expected = "must be a string")]
fn a_non_string_filter_is_rejected(#[case] document: &str) {
    let table: Value = table_from(document);

    let _ignored = filter_of(&table);
}

/// The same rejection must reach the resolver, which is what the guards call.
#[test]
#[should_panic(expected = "must be a string")]
fn a_non_string_filter_is_rejected_during_resolution() {
    let table: Value = table_from("filter = 42\nslow-timeout = \"75m\"");

    let _ignored = governing_override(&[table], "slow-timeout", sample_identity());
}

/// A filter the model cannot evaluate must stop the resolver rather than be
/// treated as matching every test.
#[test]
#[should_panic(expected = "should be of the form")]
fn an_unparsable_filter_is_rejected_during_resolution() {
    let table: Value = table_from("filter = \"package\"\nslow-timeout = \"75m\"");

    let _ignored = governing_override(&[table], "slow-timeout", sample_identity());
}
