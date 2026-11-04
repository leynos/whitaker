//! Tracks AST wrapper depth while traversing expanded syntax.
//!
//! A captured literal can retain call-site hygiene even when a macro-created
//! statement surrounds it. This visitor records only candidates with no
//! expansion-derived ancestor; later lint hooks use that positive set as the
//! gate, so a missed container suppresses a finding instead of widening it.

use std::collections::HashSet;

use rustc_ast::{
    ast::{self, ExprKind, ItemKind},
    token,
    visit::{self, Visitor},
};
use rustc_span::Span;
use whitaker_common::SimplePath;

use crate::exclusion::PathExclusions;

/// Positive safe-span set consumed by the plain-literal check.
#[derive(Default)]
pub(crate) struct SafeSpans {
    /// Plain string-expression spans with no expansion-derived ancestor.
    pub(crate) plain_literals: HashSet<Span>,
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
    }
}

struct WrapperDepth<'a> {
    expansion_depth: Option<usize>,
    module_path: Vec<String>,
    path_exclusions: &'a PathExclusions,
    safe_literal_spans: HashSet<Span>,
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
    fn new(crate_name: &str, path_exclusions: &'a PathExclusions) -> Self {
        Self {
            expansion_depth: Some(0),
            module_path: vec![crate_name.to_owned()],
            path_exclusions,
            safe_literal_spans: HashSet::new(),
        }
    }

    fn with_expansion(&mut self, is_expansion: bool, visit: impl FnOnce(&mut Self)) {
        let previous_depth = self.expansion_depth;
        if is_expansion {
            self.expansion_depth = previous_depth.and_then(|depth| depth.checked_add(1));
        }
        visit(self);
        self.expansion_depth = previous_depth;
    }

    fn with_span(&mut self, span: Span, visit: impl FnOnce(&mut Self)) {
        self.with_expansion(span.from_expansion(), visit);
    }

    fn records_plain_candidate(&mut self, expression: &ast::Expr) {
        let ExprKind::Lit(literal) = &expression.kind else {
            return;
        };
        if literal.kind != token::LitKind::Str
            || literal.suffix.is_some()
            || !literal.symbol.as_str().contains("\\\n")
            || self.expansion_depth != Some(0)
            || self.is_path_excluded()
        {
            return;
        }
        self.safe_literal_spans.insert(expression.span);
    }

    fn is_path_excluded(&self) -> bool {
        let module_path = self.module_path.join("::");
        self.path_exclusions
            .excludes(&SimplePath::parse(&module_path))
    }
}

impl<'ast> Visitor<'ast> for WrapperDepth<'_> {
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

    fn visit_assoc_item(&mut self, item: &'ast ast::AssocItem, context: visit::AssocCtxt) {
        self.with_span(item.span, |visitor| {
            visit::walk_assoc_item(visitor, item, context);
        });
    }

    visit_with_span!(visit_foreign_item, item: ForeignItem, item.span, walk_item);

    fn visit_expr(&mut self, expression: &'ast ast::Expr) {
        self.records_plain_candidate(expression);
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

    fn visit_generics(&mut self, generics: &'ast ast::Generics) {
        self.with_span(generics.span, |visitor| {
            visitor.with_span(generics.where_clause.span, |visitor| {
                visit::walk_generics(visitor, generics);
            });
        });
    }

    fn visit_fn(
        &mut self,
        kind: visit::FnKind<'ast>,
        _attrs: &ast::AttrVec,
        span: Span,
        _id: ast::NodeId,
    ) {
        let span = match &kind {
            visit::FnKind::Fn(_, _, function) => function.sig.span,
            visit::FnKind::Closure(..) => span,
        };
        self.with_span(span, |visitor| visit::walk_fn(visitor, kind));
    }

    visit_with_span!(visit_where_predicate, predicate: WherePredicate, predicate.span, walk_where_predicate);
    visit_with_span!(visit_path, path: Path, path.span, walk_path);
    visit_with_span!(visit_path_segment, segment: PathSegment, segment.span(), walk_path_segment);
    visit_with_span!(visit_generic_args, args: GenericArgs, args.span(), walk_generic_args);
    visit_with_span!(visit_assoc_item_constraint, constraint: AssocItemConstraint, constraint.span, walk_assoc_item_constraint);
    visit_with_span!(visit_poly_trait_ref, trait_ref: PolyTraitRef, trait_ref.span, walk_poly_trait_ref);
    visit_with_span!(visit_format_args, format_args: FormatArgs, format_args.span, walk_format_args);

    fn visit_ty_pat(&mut self, _pattern: &'ast ast::TyPat) {}
}

#[cfg(test)]
#[path = "../../tests/support/depth_tests.rs"]
mod tests;
