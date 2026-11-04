//! A small evaluator for the subset of the nextest filter grammar that
//! `.config/nextest.toml` uses.
//!
//! The overrides guard cannot search filter text for a package name. A
//! substring search cannot tell "this clause matches the test" from "this
//! clause matches a *different* test with the same name in another package",
//! and that ambiguity is exactly how a broad override silently shadowed a
//! package override. Evaluating the real grammar against a concrete
//! [`TestIdentity`] answers the question the override actually asks.
//!
//! Only the operators this repository writes are handled: `&`, `|`, `not`, and
//! parentheses. An unknown operator or leaf shape panics rather than answering
//! `false`, so a filter that grows a construct this module does not model fails
//! the guard loudly instead of passing vacuously.

use std::fmt;

/// A nextest profile name such as `default` or `ci`.
#[derive(Clone, Copy, Debug)]
pub struct ProfileName<'a>(pub &'a str);

/// A cargo package name, for example `no_unwrap_or_else_panic`.
#[derive(Clone, Copy, Debug)]
pub struct PackageName<'a>(pub &'a str);

impl fmt::Display for PackageName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

/// The name of a test binary, which defaults to the package name for a
/// library's inline tests and to the file stem for an integration test.
#[derive(Clone, Copy, Debug)]
pub struct BinaryName<'a>(pub &'a str);

/// The name nextest reports for a test, e.g. `ui::example_compiles`.
#[derive(Clone, Copy, Debug)]
pub struct TestName<'a>(pub &'a str);

/// A nextest filter expression such as `package(foo) & binary(foo)`.
#[derive(Clone, Copy, Debug)]
pub struct FilterExpression<'a>(pub &'a str);

/// The operator of a leaf filter term, such as `package` or `test`.
///
/// Named separately from [`FilterArgument`] because the two are adjacent in
/// [`TestIdentity::matches`] and [`split_leaf`], where a plain pair of `&str`
/// would let a caller transpose them and still compile.
#[derive(Clone, Copy, Debug)]
pub struct Operator<'a>(pub &'a str);

/// The argument of a leaf filter term, such as `no_unwrap_or_else_panic`.
#[derive(Clone, Copy, Debug)]
pub struct FilterArgument<'a>(pub &'a str);

/// One test as nextest identifies it when it evaluates an override.
#[derive(Clone, Copy, Debug)]
pub struct TestIdentity<'a> {
    /// The package that owns the test.
    pub package: PackageName<'a>,
    /// The test binary nextest runs it from.
    pub binary: BinaryName<'a>,
    /// The name nextest reports for it.
    pub name: TestName<'a>,
}

impl TestIdentity<'_> {
    /// Returns whether the single filter term `operator(argument)` selects
    /// this identity.
    ///
    /// nextest gives the operators different default matchers. `test` defaults
    /// to the *contains* matcher, so its argument matches a substring of the
    /// reported name. `package` and `binary` default to the *glob* matcher, and
    /// a glob with no metacharacters is an exact match, so their arguments must
    /// name the package or binary in full. Both operators still honour an
    /// explicit `=` prefix. An operator this repository does not use panics
    /// rather than answering `false`, so a filter that grows a new operator
    /// fails loudly instead of silently matching nothing.
    fn matches(self, operator: Operator<'_>, argument: FilterArgument<'_>) -> bool {
        match operator.0 {
            "package" => glob_names(self.package.0, argument),
            "binary" => glob_names(self.binary.0, argument),
            "test" => names(self.name.0, argument),
            other => panic!("unsupported nextest filter operator {other:?}"),
        }
    }
}

impl FilterExpression<'_> {
    /// Returns whether this expression selects `identity`.
    ///
    /// Each operator keeps its own default matcher. A bare `test(<name>)`
    /// matches a substring, so it reaches a same-named test in *every* package;
    /// the regression tests depend on that behaviour, because it is what made
    /// the broad override shadow the package overrides. `package` and `binary`
    /// instead match exactly, because nextest's default matcher for them is the
    /// glob matcher.
    #[must_use]
    pub fn selects(&self, identity: TestIdentity<'_>) -> bool { term_holds(self.0, identity) }
}

/// Returns whether `actual` is selected by a single `test` argument.
fn names(actual: &str, FilterArgument(argument): FilterArgument<'_>) -> bool {
    argument
        .strip_prefix('=')
        .map_or_else(|| actual.contains(argument), |exact| actual == exact)
}

/// Returns whether `actual` is selected by a single `package` or `binary`
/// argument.
///
/// nextest's default matcher for these operators is the glob matcher, so a
/// bare argument is an exact name unless it contains a glob metacharacter. This
/// model deliberately does not implement glob expansion: a clause that starts
/// with a matcher prefix (`~`, `/`, `#`) or contains a metacharacter is not
/// something the guards currently assert, and answering `false` for it would
/// silently pass a guard that no longer describes the configuration. Panicking
/// keeps the module's fail-loudly contract, and the panic names the argument so
/// the guard can be extended before that clause lands.
fn glob_names(actual: &str, FilterArgument(argument): FilterArgument<'_>) -> bool {
    let is_unsupported =
        argument.starts_with(['~', '/', '#']) || argument.contains(['*', '?', '[', '{']);
    assert!(
        !is_unsupported,
        "unsupported nextest glob filter argument {argument:?}"
    );

    argument
        .strip_prefix('=')
        .map_or_else(|| actual == argument, |exact| actual == exact)
}

/// Evaluates one term, which may be negated, compound, or parenthesized.
fn term_holds(term: &str, identity: TestIdentity<'_>) -> bool {
    let bare = strip_wrapping_parens(term.trim());

    // `|` binds loosest, so it is split first; then `&`, then `not`, then a
    // leaf term. `not` binds tighter than both operators, so it must be
    // unwrapped only after they have been split; unwrapping it first would
    // read `not a & b` as `not (a & b)`.
    let alternatives = split_top_level(bare, '|');
    if alternatives.len() > 1 {
        return alternatives
            .iter()
            .any(|alternative| term_holds(alternative, identity));
    }

    let conjuncts = split_top_level(bare, '&');
    if conjuncts.len() > 1 {
        return conjuncts
            .iter()
            .all(|conjunct| term_holds(conjunct, identity));
    }

    if let Some(negated) = bare.strip_prefix("not ") {
        return !term_holds(negated, identity);
    }

    let (operator, argument) = split_leaf(bare);
    identity.matches(operator, argument)
}

/// Splits `expression` on `separator` where it is not nested in parentheses.
///
/// The scan is over `char_indices`, but the slices are taken through `get`, so
/// a multi-byte character in a filter argument cannot panic the guard that
/// exists to protect the build.
fn split_top_level(expression: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut start: usize = 0;

    for (index, character) in expression.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if character == separator && depth == 0 => {
                parts.push(expression.get(start..index).unwrap_or_default().trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(expression.get(start..).unwrap_or_default().trim());
    parts
}

/// Removes parentheses that enclose the whole of `term`.
fn strip_wrapping_parens(mut term: &str) -> &str {
    while let Some(inner) = term
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
    {
        // Only unwrap when the leading `(` matches the trailing `)`; in
        // `(a) & (b)` the first `)` closes early, and stripping then would
        // silently change the meaning of the expression.
        if split_top_level(term, '&').len() > 1 || split_top_level(term, '|').len() > 1 {
            return term;
        }
        term = inner.trim();
    }

    term
}

/// Splits a leaf term such as `binary(ui)` into its operator and argument.
fn split_leaf(term: &str) -> (Operator<'_>, FilterArgument<'_>) {
    let Some(open) = term.find('(') else {
        panic!("nextest filter term {term:?} should be of the form `operator(argument)`");
    };
    let (operator, rest) = term.split_at(open);
    let Some(argument) = rest
        .strip_prefix('(')
        .and_then(|text| text.strip_suffix(')'))
    else {
        panic!("nextest filter term {term:?} should end with a closing parenthesis");
    };

    (Operator(operator.trim()), FilterArgument(argument))
}
