//! Fixtures for the crate-wide built-in macro shadow detector.
//!
//! Every case builds the smallest syntax tree that reaches one detector arm, so
//! a regression names the arm it broke rather than a whole-crate silence. The
//! renames matter: a rename changes the name a `use` binds, and only the bound
//! name can shadow a built-in.
//!
//! Symbol interning needs the compiler's session globals, so the crate is built
//! inside `create_default_session_globals_then`. The builder closes over the
//! fixture rather than taking finished items, keeping interning inside the
//! session it requires.

use rustc_ast::{
    ast::{
        self,
        AttrStyle,
        AttrVec,
        DUMMY_NODE_ID,
        DelimArgs,
        Inline,
        ItemKind,
        MacroDef,
        ModKind,
        ModSpans,
        Path,
        PathSegment,
        Safety,
        UseTree,
        UseTreeKind,
        Visibility,
        VisibilityKind,
    },
    attr::{self, AttrIdGenerator},
    token::Delimiter,
    tokenstream::{DelimSpan, TokenStream},
};
use rustc_span::{DUMMY_SP, Ident, Symbol, create_default_session_globals_then, sym};

use super::shadows_builtin_macros;

/// Reports whether the fixture built by `build` trips the detector.
fn detects_shadowing(build: impl FnOnce() -> Vec<ast::Item>) -> bool {
    create_default_session_globals_then(|| shadows_builtin_macros(&krate(build())))
}

fn empty_attrs() -> AttrVec { AttrVec::new() }

fn item(kind: ItemKind) -> ast::Item {
    ast::Item {
        attrs: empty_attrs(),
        id: DUMMY_NODE_ID,
        span: DUMMY_SP,
        vis: Visibility {
            kind: VisibilityKind::Inherited,
            span: DUMMY_SP,
            tokens: None,
        },
        kind,
        tokens: None,
    }
}

/// Wraps fixture items in the crate root.
///
/// The fixtures stay unboxed so the builders read as plain lists; the
/// `ThinVec<Box<Item>>` the AST actually stores is formed here.
fn krate(items: Vec<ast::Item>) -> ast::Crate {
    ast::Crate {
        id: DUMMY_NODE_ID,
        attrs: empty_attrs(),
        items: items.into_iter().map(Box::new).collect::<Vec<_>>().into(),
        spans: ModSpans::default(),
        is_placeholder: false,
    }
}

/// A macro or module name a fixture mentions.
///
/// The builders take this rather than `&str` so a raw string is accepted at
/// exactly one place — the interning boundary — and every name reaching the
/// syntax tree is one the test meant to write as an identifier.
#[derive(Clone, Copy)]
struct Name(&'static str);

impl Name {
    fn ident(self) -> Ident { Ident::with_dummy_span(Symbol::intern(self.0)) }
}

impl From<&'static str> for Name {
    fn from(name: &'static str) -> Self { Self(name) }
}

fn path(segments: Name) -> Path {
    let segments = segments
        .0
        .split("::")
        .map(|segment| PathSegment::from_ident(Name::from(segment).ident()))
        .collect::<Vec<_>>();
    Path {
        span: DUMMY_SP,
        segments: segments.into(),
        tokens: None,
    }
}

fn macro_def(name: Name) -> ItemKind {
    ItemKind::MacroDef(
        name.ident(),
        MacroDef {
            body: Box::new(DelimArgs {
                dspan: DelimSpan::from_single(DUMMY_SP),
                delim: Delimiter::Brace,
                tokens: TokenStream::new(Vec::new()),
            }),
            macro_rules: true,
            eii_declaration: None,
        },
    )
}

fn use_tree(prefix: Name, kind: UseTreeKind) -> ItemKind {
    ItemKind::Use(UseTree {
        prefix: path(prefix),
        kind,
    })
}

fn tree(prefix: Name, kind: UseTreeKind) -> UseTree {
    UseTree {
        prefix: path(prefix),
        kind,
    }
}

fn simple(rename: Option<Name>) -> UseTreeKind { UseTreeKind::Simple(rename.map(Name::ident)) }

fn nested(trees: Vec<UseTree>) -> UseTreeKind {
    let items = trees
        .into_iter()
        .map(|tree| (tree, DUMMY_NODE_ID))
        .collect::<Vec<_>>();
    UseTreeKind::Nested {
        items: items.into(),
        span: DUMMY_SP,
    }
}

fn extern_crate(name: Name, has_macro_use: bool) -> ast::Item {
    let mut crate_item = item(ItemKind::ExternCrate(None, name.ident()));
    if has_macro_use {
        let generator = AttrIdGenerator::new();
        crate_item.attrs.push(attr::mk_attr_word(
            &generator,
            AttrStyle::Outer,
            Safety::Default,
            sym::macro_use,
            DUMMY_SP,
        ));
    }
    crate_item
}

fn inline_module(name: Name, inner: ast::Item) -> ast::Item {
    let items = vec![Box::new(inner)].into();
    item(ItemKind::Mod(
        Safety::Default,
        name.ident(),
        ModKind::Loaded(items, Inline::Yes, ModSpans::default()),
    ))
}

#[test]
fn an_ordinary_crate_is_not_shadowed() {
    let is_shadowed = detects_shadowing(|| {
        let leaf = item(ItemKind::Mod(
            Safety::Default,
            Name::from("leaf").ident(),
            ModKind::Unloaded,
        ));
        vec![inline_module("inner".into(), leaf)]
    });
    assert!(!is_shadowed);
}

#[test]
fn a_macro_rules_definition_of_a_builtin_name_shadows() {
    assert!(detects_shadowing(|| vec![item(macro_def(
        "println".into()
    ))]));
}

#[test]
fn a_definition_reaching_concat_shadows() {
    assert!(detects_shadowing(|| vec![item(macro_def("concat".into()))]));
}

#[test]
fn a_definition_of_an_unlisted_name_does_not_shadow() {
    assert!(!detects_shadowing(|| vec![item(macro_def("shout".into()))]));
}

#[test]
fn a_use_binding_a_builtin_name_shadows() {
    assert!(detects_shadowing(|| vec![item(use_tree(
        "helpers::println".into(),
        simple(None)
    ))]));
}

#[test]
fn a_rename_onto_a_builtin_name_shadows() {
    assert!(detects_shadowing(|| vec![item(use_tree(
        "helpers::shout".into(),
        simple(Some("format_args".into()))
    ))]));
}

#[test]
fn a_rename_away_from_a_builtin_name_does_not_shadow() {
    // `use helpers::println as shout` binds `shout`; the segment it was
    // imported under can no longer reach `println!`.
    assert!(!detects_shadowing(|| vec![item(use_tree(
        "helpers::println".into(),
        simple(Some("shout".into()))
    ))]));
}

#[test]
fn a_nested_use_tree_is_searched() {
    let is_shadowed = detects_shadowing(|| {
        let branches = nested(vec![
            tree("helpers::print".into(), simple(None)),
            tree("helpers::shout".into(), simple(None)),
        ]);
        vec![item(use_tree("helpers".into(), branches))]
    });
    assert!(is_shadowed);
}

#[test]
fn a_glob_import_is_accepted_as_a_residual_hole() {
    let is_shadowed = detects_shadowing(|| {
        vec![item(use_tree(
            "helpers".into(),
            UseTreeKind::Glob(DUMMY_SP),
        ))]
    });
    assert!(!is_shadowed);
}

#[test]
fn an_extern_crate_with_macro_use_shadows() {
    assert!(detects_shadowing(|| vec![extern_crate(
        "helpers".into(),
        true
    )]));
}

#[test]
fn an_extern_crate_without_macro_use_does_not_shadow() {
    assert!(!detects_shadowing(|| vec![extern_crate(
        "helpers".into(),
        false
    )]));
}

#[test]
fn a_definition_nested_in_a_module_is_seen() {
    let is_shadowed = detects_shadowing(|| {
        vec![inline_module(
            "inner".into(),
            item(macro_def("assert_eq".into())),
        )]
    });
    assert!(is_shadowed);
}
