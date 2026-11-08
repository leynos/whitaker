//! Tracks AST wrapper depth while traversing expanded syntax.
//!
//! A captured literal can retain call-site hygiene even when a macro-created
//! statement surrounds it. This visitor records only candidates with no
//! expansion-derived ancestor; later lint hooks use that positive set as the
//! gate, so a missed container suppresses a finding instead of widening it.

use std::collections::HashSet;

use rustc_ast::{
    ast::{self, ItemKind},
    visit::{self, Visitor},
};
use rustc_span::Span;
use whitaker_common::SimplePath;

use super::{
    literal::{join_format_string, outermost_macro_name, plain_join_literal},
    shadow::is_format_macro_name,
};
use crate::exclusion::PathExclusions;

/// Positive safe-span set consumed by the expression checks.
#[derive(Default)]
pub(crate) struct SafeSpans {
    /// Plain string-expression spans with no expansion-derived ancestor.
    pub(crate) plain_literals: HashSet<Span>,
    /// Source-authored format-string token spans reached through a format
    /// macro, with no expansion-derived ancestor of their own.
    pub(crate) format_strings: HashSet<Span>,
}

/// Finds source literal expressions that are not wrapped by macro expansion.
pub(crate) fn safe_literal_spans(
    krate: &ast::Crate,
    crate_name: &str,
    path_exclusions: &PathExclusions,
) -> SafeSpans {
    let mut visitor = WrapperDepth::new(crate_name, path_exclusions);
    visitor.visit_crate(krate);
    SafeSpans {
        plain_literals: visitor.safe_literal_spans,
        format_strings: visitor.safe_format_spans,
    }
}

struct WrapperDepth<'a> {
    expansion_depth: Option<usize>,
    module_path: Vec<String>,
    path_exclusions: &'a PathExclusions,
    safe_literal_spans: HashSet<Span>,
    safe_format_spans: HashSet<Span>,
}

// Keep the repeated span/walk shape local to this visitor; the invocation list
// is the auditable set of AST containers that can hide a source literal.
macro_rules! visit_with_span {
    ($method:ident, $node:ident : $ty:ident, $span:expr, $walk:ident) => {
        fn $method(&mut self, $node: &'ast ast::$ty) {
            self.with_span($span, |visitor| visit::$walk(visitor, $node));
        }
    };
}

impl<'a> WrapperDepth<'a> {
    /// Starts a walk at source depth with the crate as the first path segment.
    fn new(crate_name: &str, path_exclusions: &'a PathExclusions) -> Self {
        Self {
            expansion_depth: Some(0),
            module_path: vec![crate_name.to_owned()],
            path_exclusions,
            safe_literal_spans: HashSet::new(),
            safe_format_spans: HashSet::new(),
        }
    }

    /// Runs `visit` one expansion level deeper than the current depth.
    ///
    /// Depth saturates at `None` rather than wrapping: once an expansion chain
    /// is deeper than `usize` can count, the answer is "cannot tell", and
    /// [`Self::is_at_source_depth`] must treat that as not-at-source-depth.
    fn with_expansion(&mut self, is_expansion: bool, visit: impl FnOnce(&mut Self)) {
        let previous_depth = self.expansion_depth;
        if is_expansion {
            self.expansion_depth = previous_depth.and_then(|depth| depth.checked_add(1));
        }
        visit(self);
        self.expansion_depth = previous_depth;
    }

    /// Runs `visit` at a depth one greater when `span` came from expansion.
    fn with_span(&mut self, span: Span, visit: impl FnOnce(&mut Self)) {
        self.with_expansion(span.from_expansion(), visit);
    }

    /// Records a plain literal expression that no wrapper encloses.
    fn records_plain_candidate(&mut self, expression: &ast::Expr) {
        if plain_join_literal(expression).is_none() {
            return;
        }
        if !self.is_at_source_depth() {
            return;
        }
        self.safe_literal_spans.insert(expression.span);
    }

    /// Records a format-string candidate.
    ///
    /// The wrapper-depth test is deliberately *not* applied here, and the
    /// reason is structural rather than a relaxation. A format literal is never
    /// at depth zero: the facade chain (`println` → `print_to` →
    /// `format_args_nl` → the builtin) is made entirely of expansions, and the
    /// builtin def-site-marks the expression it builds. Measured on a direct
    /// `println!("alpha \<newline> beta{}", value)`, the depth at this hook is
    /// five.
    ///
    /// Depth zero stood for "no enclosing matcher can be broken". That property
    /// is proved here instead by the expansion chain of the enclosing
    /// expression: if the outermost frame is a known format macro, then every
    /// frame between the user's invocation and the builtin is one of them, so
    /// no user macro sits in between. That is a stronger statement than depth
    /// zero, and it also closes the `$literal` hole: a `shout!(…)` that
    /// forwards its tokens to `println!` leaves the literal token's own chain
    /// at root, but the builtin's def-site span walks back out through
    /// `println`'s frame to `shout`'s.
    fn records_format_candidate(&mut self, expression: &ast::Expr, arguments: &ast::FormatArgs) {
        if join_format_string(arguments).is_none() {
            return;
        }
        if self.is_path_excluded() {
            return;
        }
        let Some(macro_name) = outermost_macro_name(expression.span) else {
            return;
        };
        if !is_format_macro_name(macro_name.as_str()) {
            return;
        }
        self.safe_format_spans.insert(arguments.span);
    }

    /// Reports whether nothing expansion-derived encloses the current node.
    fn is_at_source_depth(&self) -> bool {
        self.expansion_depth == Some(0) && !self.is_path_excluded()
    }

    /// Reports whether the current module path matches a configured exclusion.
    fn is_path_excluded(&self) -> bool {
        let module_path = self.module_path.join("::");
        self.path_exclusions
            .excludes(&SimplePath::parse(&module_path))
    }
}

impl<'ast> Visitor<'ast> for WrapperDepth<'_> {
    /// Walks an item, tracking the module path for `excluded_paths`.
    fn visit_item(&mut self, item: &'ast ast::Item) {
        let previous_path_len = self.module_path.len();
        self.with_span(item.span, |visitor| {
            if let ItemKind::Mod(_, ident, _) = &item.kind {
                visitor.module_path.push(ident.name.as_str().to_owned());
            }
            visit::walk_item(visitor, item);
            visitor.module_path.truncate(previous_path_len);
        });
    }

    /// Walks an associated item one depth deeper when its span is expanded.
    fn visit_assoc_item(&mut self, item: &'ast ast::AssocItem, context: visit::AssocCtxt) {
        self.with_span(item.span, |visitor| {
            visit::walk_assoc_item(visitor, item, context);
        });
    }

    visit_with_span!(visit_foreign_item, item: ForeignItem, item.span, walk_item);

    /// Records candidates at this expression, then descends one level deeper.
    ///
    /// Ordering matters: both candidates are recorded before the enclosing
    /// expression's own span joins the depth count. A format expression's span
    /// is def-site-marked by the builtin, so counting it first would suppress
    /// every format string in the crate.
    fn visit_expr(&mut self, expression: &'ast ast::Expr) {
        self.records_plain_candidate(expression);
        if let ast::ExprKind::FormatArgs(arguments) = &expression.kind {
            self.records_format_candidate(expression, arguments);
        }
        self.with_span(expression.span, |visitor| {
            visit::walk_expr(visitor, expression);
        });
    }

    visit_with_span!(visit_stmt, statement: Stmt, statement.span, walk_stmt);
    visit_with_span!(visit_local, local: Local, local.span, walk_local);
    visit_with_span!(visit_block, block: Block, block.span, walk_block);
    visit_with_span!(visit_arm, arm: Arm, arm.span, walk_arm);
    visit_with_span!(visit_expr_field, field: ExprField, field.span, walk_expr_field);
    visit_with_span!(visit_field_def, field: FieldDef, field.span, walk_field_def);
    visit_with_span!(visit_variant, variant: Variant, variant.span, walk_variant);

    /// Walks a pattern, skipping literal and range patterns entirely.
    fn visit_pat(&mut self, pattern: &'ast ast::Pat) {
        // Literal patterns are not replaceable value expressions.
        if matches!(
            &pattern.kind,
            ast::PatKind::Expr(_) | ast::PatKind::Range(..)
        ) {
            return;
        }
        self.with_span(pattern.span, |visitor| visit::walk_pat(visitor, pattern));
    }

    visit_with_span!(visit_ty, ty: Ty, ty.span, walk_ty);
    visit_with_span!(visit_param, param: Param, param.span, walk_param);
    visit_with_span!(visit_generic_param, param: GenericParam, param.span(), walk_generic_param);

    /// Walks generics, counting the where clause as its own wrapper.
    fn visit_generics(&mut self, generics: &'ast ast::Generics) {
        self.with_span(generics.span, |with_generics| {
            with_generics.with_span(generics.where_clause.span, |with_where_clause| {
                visit::walk_generics(with_where_clause, generics);
            });
        });
    }

    /// Walks a function, using the signature span rather than the whole item.
    ///
    /// The five-parameter shape is the `rustc_ast::visit::Visitor` trait's own
    // signature; it cannot be grouped into a struct without ceasing to be that
    // trait method, which would drop the walk. `_attrs` and `_id` are unused
    // because only the function's signature span and its body matter here.
    // @codescene(disable:"Excess Number of Function Arguments") trait signature (2026-11-05)
    fn visit_fn(
        &mut self,
        kind: visit::FnKind<'ast>,
        _attrs: &ast::AttrVec,
        span: Span,
        _id: ast::NodeId,
    ) {
        let effective_span = match &kind {
            visit::FnKind::Fn(_, _, function) => function.sig.span,
            visit::FnKind::Closure(..) => span,
        };
        self.with_span(effective_span, |visitor| visit::walk_fn(visitor, kind));
    }

    visit_with_span!(visit_where_predicate, predicate: WherePredicate, predicate.span, walk_where_predicate);
    visit_with_span!(visit_path, path: Path, path.span, walk_path);
    visit_with_span!(visit_path_segment, segment: PathSegment, segment.span(), walk_path_segment);
    visit_with_span!(visit_generic_args, args: GenericArgs, args.span(), walk_generic_args);
    visit_with_span!(visit_assoc_item_constraint, constraint: AssocItemConstraint, constraint.span, walk_assoc_item_constraint);
    visit_with_span!(visit_poly_trait_ref, trait_ref: PolyTraitRef, trait_ref.span, walk_poly_trait_ref);
    visit_with_span!(visit_format_args, format_args: FormatArgs, format_args.span, walk_format_args);

    /// Skips attributes, whose values are never replaceable expressions.
    ///
    /// `#[doc = "..."]` and friends carry their value as a meta-item literal,
    /// so walking them would record a span the lint hook must then decline to
    /// rewrite. `#[cfg]` is the only attribute that reaches macro expansion,
    /// where it is consumed rather than retained, so skipping the interior
    /// costs no real coverage.
    fn visit_attribute(&mut self, _attr: &'ast ast::Attribute) {}

    /// Skips type-position patterns, which are not value expressions.
    fn visit_ty_pat(&mut self, _pattern: &'ast ast::TyPat) {}
}

#[cfg(test)]
#[path = "../../tests/support/depth_tests.rs"]
mod tests;
