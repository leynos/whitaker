//! Detects crate-wide shadowing of the compiler built-in macro names.
//!
//! The lint names built-in macros without a module qualifier: the format
//! family whose invocation position proves a literal sits in expression
//! position, and `concat!` in the replacement it suggests. A bare name is only
//! sound to report when the crate cannot substitute its own macro for it, and
//! a post-expansion early pass has no name resolution to consult. Detection is
//! therefore syntactic, crate-wide, and deliberately blunt: any matching
//! `macro_rules!`/`macro` definition, any `use` that binds a matching name, any
//! glob import, or any `#[macro_use] extern crate` silences the lint for the
//! whole crate.
//!
//! Detection does not model textual or module scope. A definition that could
//! not shadow a given call site still silences the crate. The bias is
//! deliberate: a false negative is invisible, while a false positive rewrites
//! working code into a call to something else.

use rustc_ast::{
    ast::{self, ItemKind, UseTreeKind},
    visit::{self, Visitor},
};
use rustc_span::{Symbol, sym};

/// Format macros whose invocation position proves a literal is in expression
/// position.
///
/// The set is the RFC's fixed list. It must not grow to cover `concat`: a
/// format-string literal reached through a user `concat!` is not in format
/// position, and [`is_builtin_macro_name`] adds that name only for the
/// shadowing check.
pub(crate) const FORMAT_MACRO_NAMES: [&str; 18] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "eprint",
    "eprintln",
    "format",
    "format_args",
    "panic",
    "print",
    "println",
    "todo",
    "unimplemented",
    "unreachable",
    "write",
    "writeln",
];

/// Reports whether `name` is one of the RFC's format-macro names.
pub(crate) fn is_format_macro_name(name: &str) -> bool { FORMAT_MACRO_NAMES.contains(&name) }

/// Reports whether `krate` can substitute a macro named in the built-in set.
pub(crate) fn shadows_builtin_macros(krate: &ast::Crate) -> bool {
    let mut detector = ShadowDetector { is_shadowed: false };
    detector.visit_crate(krate);
    detector.is_shadowed
}

/// Reports whether `name` is one the lint spells without a module qualifier.
///
/// This is the format-macro set plus `concat`, which the suggested
/// replacement itself names. The shadowing check must cover `concat` even
/// though the format-position check must not.
pub(crate) fn is_builtin_macro_name(name: &str) -> bool {
    name == "concat" || is_format_macro_name(name)
}

struct ShadowDetector {
    is_shadowed: bool,
}

impl ShadowDetector {
    /// Records a name the crate binds, if it is one the lint spells bare.
    fn record_name(&mut self, name: Symbol) {
        self.is_shadowed |= is_builtin_macro_name(name.as_str());
    }

    /// Records an `extern crate` as shadowing when it carries `#[macro_use]`.
    ///
    /// Only `#[macro_use]` imports that crate's macro namespace into the
    /// crate root; a plain `extern crate` binds the crate name alone.
    fn record_extern_crate(&mut self, item: &ast::Item) {
        self.is_shadowed |= item
            .attrs
            .iter()
            .any(|attribute| attribute.has_name(sym::macro_use));
    }

    /// Records a glob import as shadowing, unless the compiler injected it.
    ///
    /// Every crate is handed `use std::prelude::rust_20xx::*;` before the
    /// user's items, and that injection is built on `DUMMY_SP`. Counting it
    /// would silence the lint in every crate, which is not a conservative
    /// reading of the ambiguity risk — it is the detector switching itself off
    /// everywhere. A glob that appears in source, whether written directly or
    /// produced by a macro, has a real location, and only such a glob can carry
    /// a `concat` binding the suggested call would collide with.
    fn record_glob(&mut self, tree: &ast::UseTree) { self.is_shadowed |= !tree.span().is_dummy(); }

    /// Records the name a `use` tree actually binds, recursing into nesting.
    fn record_use_tree(&mut self, tree: &ast::UseTree) {
        match &tree.kind {
            // A rename makes the source segment irrelevant: `use p::println as
            // shout` binds `shout` and can never shadow `println!`. The name
            // the tree actually binds is the rename when there is one.
            UseTreeKind::Simple(rename) => {
                let bound = rename.unwrap_or_else(|| tree.ident());
                self.record_name(bound.name);
            }
            UseTreeKind::Nested { items, .. } => {
                for (nested, _) in items {
                    self.record_use_tree(nested);
                }
            }
            // `use module::*` has no segment to inspect, and it can bind
            // `concat`. A glob-imported `concat` does not shadow it outright,
            // as a definition would; it makes the name *ambiguous*, so the
            // suggested bare `concat!(...)` fails with E0659 at the call site
            // the lint just rewrote. That is the false positive this detector
            // exists to prevent, caused by a construct the detector cannot
            // resolve without name resolution. Treating a source-written glob
            // as shadowing is therefore the sound reading of the bias stated
            // in the module comment: silence is a cost the user can see and
            // attribute, while a broken rewrite is not.
            UseTreeKind::Glob(_) => self.record_glob(tree),
        }
    }
}

impl<'ast> Visitor<'ast> for ShadowDetector {
    /// Records macro definitions and `#[macro_use] extern crate` items.
    fn visit_item(&mut self, item: &'ast ast::Item) {
        match &item.kind {
            ItemKind::MacroDef(ident, _) => self.record_name(ident.name),
            ItemKind::ExternCrate(..) => self.record_extern_crate(item),
            _ => {}
        }
        visit::walk_item(self, item);
    }

    /// Records every name bound by a `use` declaration.
    fn visit_use_tree(&mut self, tree: &'ast ast::UseTree) { self.record_use_tree(tree); }
}

#[cfg(test)]
#[path = "../../tests/support/macro_name_tests.rs"]
mod name_tests;

#[cfg(test)]
#[path = "../../tests/support/shadow_tests.rs"]
mod tests;
