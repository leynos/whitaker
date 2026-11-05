//! Tests the format-string predicates that need no compiler session.
//!
//! `join_format_string` and `has_captured_argument` both read only the parsed
//! `FormatArgs`, so the fixtures build one directly instead of driving a
//! compilation. The `outermost_macro_name` predicate is deliberately not
//! covered here: it needs real expansion frames, and a synthesized span would
//! prove only that the fixture's context was root, not that the walk works.
//! The UI fixtures cover it end to end.

use rustc_ast::{
    ast::{
        AttrVec,
        DUMMY_NODE_ID,
        Expr,
        ExprKind,
        FormatArgs,
        FormatArgument,
        FormatArgumentKind,
        FormatArguments,
    },
    token::LitKind,
};
use rustc_span::{DUMMY_SP, Ident, Symbol, create_default_session_globals_then};

use super::{has_captured_argument, join_format_string};

fn dummy_expr() -> Box<Expr> {
    Box::new(Expr {
        id: DUMMY_NODE_ID,
        kind: ExprKind::Dummy,
        span: DUMMY_SP,
        attrs: AttrVec::new(),
        tokens: None,
    })
}

/// Builds the smallest `FormatArgs` the predicates inspect.
fn format_args(body: &str, is_source_literal: bool, arguments: Vec<FormatArgument>) -> FormatArgs {
    let mut collected = FormatArguments::new();
    for argument in arguments {
        collected.add(argument);
    }
    FormatArgs {
        span: DUMMY_SP,
        template: Vec::new(),
        arguments: collected,
        uncooked_fmt_str: (LitKind::Str, Symbol::intern(body)),
        is_source_literal,
    }
}

fn captured(name: &str) -> FormatArgument {
    FormatArgument {
        kind: FormatArgumentKind::Captured(Ident::with_dummy_span(Symbol::intern(name))),
        expr: dummy_expr(),
    }
}

fn normal() -> FormatArgument {
    FormatArgument {
        kind: FormatArgumentKind::Normal,
        expr: dummy_expr(),
    }
}

#[test]
fn a_source_literal_with_a_join_yields_its_body() {
    create_default_session_globals_then(|| {
        let arguments = format_args("alpha \\\nbeta", true, Vec::new());
        assert_eq!(join_format_string(&arguments), Some("alpha \\\nbeta"));
    });
}

#[test]
fn a_generated_format_string_is_rejected() {
    // `format!(concat!("alpha ", "beta"))` reaches the builtin with this flag
    // false. There is no direct source literal to rewrite.
    create_default_session_globals_then(|| {
        let arguments = format_args("alpha beta", false, Vec::new());
        assert_eq!(join_format_string(&arguments), None);
    });
}

#[test]
fn a_format_string_without_a_join_is_rejected() {
    create_default_session_globals_then(|| {
        let arguments = format_args("alpha beta", true, Vec::new());
        assert_eq!(join_format_string(&arguments), None);
    });
}

#[test]
fn an_empty_format_string_is_rejected() {
    create_default_session_globals_then(|| {
        let arguments = format_args("", true, Vec::new());
        assert_eq!(join_format_string(&arguments), None);
    });
}

#[test]
fn the_body_is_returned_without_its_quotes() {
    // The interning in `cook_quoted` stores only the content between the
    // quotes, so a caller comparing against a source snippet must strip quotes
    // itself. A body that still carried them would fail that comparison.
    create_default_session_globals_then(|| {
        let arguments = format_args("a \\\nb", true, Vec::new());
        let body = join_format_string(&arguments).expect("the join is present");
        assert!(!body.starts_with('"'));
        assert!(!body.ends_with('"'));
    });
}

#[test]
fn a_captured_argument_is_detected() {
    create_default_session_globals_then(|| {
        let arguments = format_args("alpha \\\nbeta{value}", true, vec![captured("value")]);
        assert!(has_captured_argument(&arguments));
    });
}

#[test]
fn a_positional_argument_is_not_a_capture() {
    create_default_session_globals_then(|| {
        let arguments = format_args("alpha \\\nbeta{}", true, vec![normal()]);
        assert!(!has_captured_argument(&arguments));
    });
}

#[test]
fn a_capture_after_an_explicit_argument_is_still_detected() {
    // Captured width and precision arrive after the explicit arguments, so a
    // predicate that stopped at the first non-captured entry would miss them.
    create_default_session_globals_then(|| {
        let arguments = format_args(
            "alpha \\\nbeta{value:width$}",
            true,
            vec![normal(), captured("width")],
        );
        assert!(has_captured_argument(&arguments));
    });
}

#[test]
fn a_format_string_with_no_arguments_has_no_capture() {
    create_default_session_globals_then(|| {
        let arguments = format_args("alpha \\\nbeta", true, Vec::new());
        assert!(!has_captured_argument(&arguments));
    });
}
