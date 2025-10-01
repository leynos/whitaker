use rustc_ast::AttrStyle;
use rustc_hir::Attribute;
use rustc_hir::attrs::AttributeKind;
use rustc_span::{Symbol, sym};

#[derive(Debug)]
pub struct AttributeOrderViolation<'a> {
    pub offending: &'a Attribute,
    pub misplaced_doc: &'a Attribute,
}

pub fn is_doc_attr(attr: &Attribute) -> bool {
    attr.is_doc_comment() || attr.has_name(sym::doc)
}

pub fn is_inner_doc(attr: &Attribute) -> bool {
    matches!(
        attr,
        Attribute::Parsed(AttributeKind::DocComment {
            style: AttrStyle::Inner,
            ..
        })
    )
}

pub fn is_outer_doc(attr: &Attribute) -> bool {
    matches!(
        attr,
        Attribute::Parsed(AttributeKind::DocComment {
            style: AttrStyle::Outer,
            ..
        })
    ) || (attr.has_name(sym::doc) && !is_inner_doc(attr))
}

pub fn doc_attrs<'a>(attrs: &'a [Attribute]) -> impl Iterator<Item = &'a Attribute> {
    attrs.iter().filter(|attr| is_doc_attr(attr))
}

pub fn non_doc_attrs<'a>(attrs: &'a [Attribute]) -> impl Iterator<Item = &'a Attribute> {
    attrs.iter().filter(|attr| !is_doc_attr(attr))
}

pub fn ensure_doc_attrs_first<'a>(attrs: &'a [Attribute]) -> Option<AttributeOrderViolation<'a>> {
    let mut first_non_doc = None;
    for attr in attrs {
        if is_doc_attr(attr) {
            if let Some(offending) = first_non_doc {
                return Some(AttributeOrderViolation {
                    offending,
                    misplaced_doc: attr,
                });
            }
        } else if first_non_doc.is_none() {
            first_non_doc = Some(attr);
        }
    }
    None
}

pub fn has_doc_attr(attrs: &[Attribute]) -> bool {
    doc_attrs(attrs).next().is_some()
}

pub fn first_non_doc_attr<'a>(attrs: &'a [Attribute]) -> Option<&'a Attribute> {
    attrs.iter().find(|attr| !is_doc_attr(attr))
}

pub fn has_test_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| is_test_attr(attr))
}

pub fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| is_cfg_test_attr(attr))
}

fn is_test_attr(attr: &Attribute) -> bool {
    attr_symbols(attr)
        .map(|segments| {
            const CANDIDATES: &[&[&str]] = &[
                &["test"],
                &["tokio", "test"],
                &["async_std", "test"],
                &["rstest"],
                &["rstest", "rstest"],
                &["test_case"],
            ];
            CANDIDATES
                .iter()
                .any(|candidate| segments_match(&segments, candidate))
        })
        .unwrap_or(false)
}

fn is_cfg_test_attr(attr: &Attribute) -> bool {
    if !attr.has_name(sym::cfg) {
        return false;
    }

    attr.meta_item_list()
        .into_iter()
        .flatten()
        .any(|meta| meta.has_name(sym::test))
}

fn attr_symbols(attr: &Attribute) -> Option<Vec<Symbol>> {
    if is_doc_attr(attr) {
        return None;
    }

    let path = attr.path();
    if path.is_empty() {
        None
    } else {
        Some(path.into_iter().collect())
    }
}

fn segments_match(segments: &[Symbol], expected: &[&str]) -> bool {
    segments.len() == expected.len()
        && segments
            .iter()
            .zip(expected.iter().copied())
            .all(|(segment, expected)| *segment == Symbol::intern(expected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use rustc_ast::ast::{
        AttrArgs as AstAttrArgs, AttrItem, AttrKind, AttrStyle, Path, PathSegment, Safety,
    };
    use rustc_ast::attr::{self, AttrIdGenerator};
    use rustc_ast::token::CommentKind;
    use rustc_span::{DUMMY_SP, Ident};

    fn generator() -> AttrIdGenerator {
        AttrIdGenerator::new()
    }

    fn with_session_globals<T>(f: impl FnOnce() -> T) -> T {
        rustc_span::create_session_globals_then(
            rustc_span::edition::Edition::Edition2024,
            &[],
            None,
            f,
        )
    }

    fn doc_comment(style: AttrStyle) -> Attribute {
        Attribute::Parsed(AttributeKind::DocComment {
            style,
            kind: CommentKind::Line,
            span: DUMMY_SP,
            comment: Symbol::intern("docs"),
        })
    }

    fn ast_attr_to_hir(attr: rustc_ast::Attribute) -> Attribute {
        match attr.kind {
            AttrKind::Normal(normal) => {
                let item = normal.item;
                let args = match item.args {
                    AstAttrArgs::Empty => rustc_hir::AttrArgs::Empty,
                    AstAttrArgs::Delimited(args) => rustc_hir::AttrArgs::Delimited(args),
                    AstAttrArgs::Eq { .. } => {
                        panic!("eq-style attributes are not used in these tests")
                    }
                };
                Attribute::Unparsed(Box::new(rustc_hir::AttrItem {
                    path: rustc_hir::AttrPath::from_ast(&item.path),
                    args,
                    id: rustc_hir::HashIgnoredAttrId { attr_id: attr.id },
                    style: attr.style,
                    span: attr.span,
                }))
            }
            AttrKind::DocComment(..) => panic!("expected a normal attribute"),
        }
    }

    fn outer_attr(id_gen: &AttrIdGenerator, name: Symbol) -> Attribute {
        let attr = attr::mk_attr_word(id_gen, AttrStyle::Outer, Safety::Default, name, DUMMY_SP);
        ast_attr_to_hir(attr)
    }

    fn path_attr(id_gen: &AttrIdGenerator, segments: &[&str]) -> Attribute {
        let path_segments = segments
            .iter()
            .map(|segment| {
                let symbol = Symbol::intern(segment);
                PathSegment::from_ident(Ident::new(symbol, DUMMY_SP))
            })
            .collect();
        let path = Path {
            span: DUMMY_SP,
            segments: path_segments,
            tokens: None,
        };
        let item = AttrItem {
            unsafety: Safety::Default,
            path,
            args: AstAttrArgs::Empty,
            tokens: None,
        };
        let attr = attr::mk_attr_from_item(id_gen, item, None, AttrStyle::Outer, DUMMY_SP);
        ast_attr_to_hir(attr)
    }

    #[rstest]
    fn detects_doc_comment() {
        with_session_globals(|| {
            let doc = doc_comment(AttrStyle::Outer);
            assert!(is_doc_attr(&doc));
            assert!(has_doc_attr(&[doc]));
        });
    }

    #[rstest]
    fn detects_order_violation() {
        with_session_globals(|| {
            let id_gen = generator();
            let attrs = [
                outer_attr(&id_gen, sym::inline),
                doc_comment(AttrStyle::Outer),
            ];
            let violation = ensure_doc_attrs_first(&attrs).expect("should detect violation");
            assert!(violation.offending.has_name(sym::inline));
        });
    }

    #[rstest]
    fn allows_docs_before_attrs() {
        with_session_globals(|| {
            let id_gen = generator();
            let attrs = [
                doc_comment(AttrStyle::Outer),
                outer_attr(&id_gen, sym::inline),
            ];
            assert!(ensure_doc_attrs_first(&attrs).is_none());
        });
    }

    #[rstest]
    fn recognises_test_attribute() {
        with_session_globals(|| {
            let id_gen = generator();
            let attrs = [outer_attr(&id_gen, sym::test)];
            assert!(has_test_marker(&attrs));
        });
    }

    #[rstest]
    fn recognises_namespaced_test_attribute() {
        with_session_globals(|| {
            let id_gen = generator();
            let attrs = [path_attr(&id_gen, &["tokio", "test"])];
            assert!(has_test_marker(&attrs));
        });
    }

    #[rstest]
    fn recognises_rstest_macro_attribute() {
        with_session_globals(|| {
            let id_gen = generator();
            let attrs = [path_attr(&id_gen, &["rstest"])];
            assert!(has_test_marker(&attrs));
        });
    }

    #[rstest]
    fn recognises_double_rstest_attribute() {
        with_session_globals(|| {
            let id_gen = generator();
            let attrs = [path_attr(&id_gen, &["rstest", "rstest"])];
            let segments = attr_symbols(&attrs[0]).expect("should expose path segments");
            assert_eq!(segments.len(), 2);
            assert_eq!(segments[0], Symbol::intern("rstest"));
            assert_eq!(segments[1], Symbol::intern("rstest"));
            assert!(has_test_marker(&attrs));
        });
    }

    #[rstest]
    fn recognises_cfg_test() {
        with_session_globals(|| {
            let id_gen = generator();
            let cfg_attr = attr::mk_attr_nested_word(
                &id_gen,
                AttrStyle::Outer,
                Safety::Default,
                sym::cfg,
                sym::test,
                DUMMY_SP,
            );
            let cfg_attr = ast_attr_to_hir(cfg_attr);
            assert!(has_cfg_test(&[cfg_attr]));
        });
    }

    #[rstest]
    fn rejects_non_test_attrs() {
        with_session_globals(|| {
            let id_gen = generator();
            let attrs = [outer_attr(&id_gen, sym::inline)];
            assert!(!has_test_marker(&attrs));
            assert!(!has_cfg_test(&attrs));
        });
    }
}
