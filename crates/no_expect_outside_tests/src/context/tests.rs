use super::{convert_attribute, meta_contains_test_cfg};
use common::AttributeKind;
use rstest::rstest;
use rustc_ast::ast::{MetaItem, MetaItemInner, MetaItemKind, Path, PathSegment, Safety};
use rustc_hir as hir;
use rustc_span::symbol::Ident;
use rustc_span::{AttrId, DUMMY_SP, create_default_session_globals_then};

fn path_from_segments(segments: &[&str]) -> Path {
    let path_segments = segments
        .iter()
        .map(|segment| PathSegment::from_ident(Ident::from_str(segment)))
        .collect::<Vec<_>>()
        .into();

    Path {
        span: DUMMY_SP,
        segments: path_segments,
        tokens: None,
    }
}

fn hir_attribute_from_segments(segments: &[&str]) -> hir::Attribute {
    create_default_session_globals_then(|| {
        let path_segments = segments
            .iter()
            .map(|segment| Ident::from_str(segment))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let attr_item = hir::AttrItem {
            path: hir::AttrPath {
                segments: path_segments,
                span: DUMMY_SP,
            },
            args: hir::AttrArgs::Empty,
            id: hir::HashIgnoredAttrId {
                attr_id: AttrId::from_u32(0),
            },
            style: rustc_ast::AttrStyle::Outer,
            span: DUMMY_SP,
        };

        hir::Attribute::Unparsed(Box::new(attr_item))
    })
}

#[rstest]
#[case(&["tokio", "test"])]
#[case(&["rstest"])]
fn convert_attribute_preserves_segments(#[case] segments: &[&str]) {
    let hir_attr = hir_attribute_from_segments(segments);
    let attribute = convert_attribute(&hir_attr);

    assert_eq!(attribute.kind(), AttributeKind::Outer);
    let converted_segments = attribute
        .path()
        .segments()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(converted_segments.as_slice(), segments);
}

fn meta_word(segments: &[&str]) -> MetaItem {
    MetaItem {
        path: path_from_segments(segments),
        kind: MetaItemKind::Word,
        span: DUMMY_SP,
        unsafety: Safety::Default,
    }
}

fn meta_list(segments: &[&str], children: Vec<MetaItemInner>) -> MetaItem {
    MetaItem {
        path: path_from_segments(segments),
        kind: MetaItemKind::List(children.into()),
        span: DUMMY_SP,
        unsafety: Safety::Default,
    }
}

fn meta_inner(meta: MetaItem) -> MetaItemInner {
    MetaItemInner::MetaItem(meta)
}

#[rstest]
#[case(meta_list(
    &["cfg"],
    vec![meta_inner(meta_list(
        &["any"],
        vec![meta_inner(meta_word(&["test"])), meta_inner(meta_word(&["doctest"]))],
    ))],
), true)]
#[case(meta_list(
    &["cfg"],
    vec![meta_inner(meta_list(
        &["all"],
        vec![meta_inner(meta_word(&["test"])), meta_inner(meta_word(&["unix"]))],
    ))],
), true)]
#[case(meta_list(
    &["cfg"],
    vec![meta_inner(meta_list(
        &["not"],
        vec![meta_inner(meta_word(&["test"]))],
    ))],
), false)]
#[case(meta_list(
    &["cfg_attr"],
    vec![
        meta_inner(meta_word(&["test"])),
        meta_inner(meta_list(&["cfg"], vec![meta_inner(meta_word(&["test"]))])),
    ],
), true)]
#[case(meta_list(
    &["cfg_attr"],
    vec![
        meta_inner(meta_word(&["test"])),
        meta_inner(meta_list(&["allow"], vec![meta_inner(meta_word(&["dead_code"]))])),
    ],
), false)]
fn meta_contains_test_cfg_cases(#[case] meta: MetaItem, #[case] expected: bool) {
    assert_eq!(meta_contains_test_cfg(&meta), expected);
}
