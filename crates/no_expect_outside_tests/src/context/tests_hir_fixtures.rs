//! HIR attribute fixtures for the test-ancestry tests.

use rustc_ast::{
    ast::DelimArgs,
    token::{Delimiter, IdentIsRaw, TokenKind},
    tokenstream::{DelimSpan, TokenStream, TokenTree},
};
use rustc_hir as hir;
use rustc_span::{AttrId, DUMMY_SP};
use whitaker_common::AttributePath;

use super::{AttributeFixture, PATH_TEST, PathSegments};

pub(super) fn hir_attribute_from_segments(segments: PathSegments) -> hir::Attribute {
    let path_segments = segments
        .as_ref()
        .iter()
        .map(|segment| rustc_span::Symbol::intern(segment))
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
}

fn hir_cfg_test_attribute() -> hir::Attribute {
    let attr_item = hir::AttrItem {
        path: hir::AttrPath {
            segments: vec![rustc_span::Symbol::intern("cfg")].into_boxed_slice(),
            span: DUMMY_SP,
        },
        args: hir::AttrArgs::Delimited(DelimArgs {
            dspan: DelimSpan::from_single(DUMMY_SP),
            delim: Delimiter::Parenthesis,
            tokens: TokenStream::new(vec![TokenTree::token_alone(
                TokenKind::Ident(rustc_span::sym::test, IdentIsRaw::No),
                DUMMY_SP,
            )]),
        }),
        id: hir::HashIgnoredAttrId {
            attr_id: AttrId::from_u32(1),
        },
        style: rustc_ast::AttrStyle::Outer,
        span: DUMMY_SP,
    };

    hir::Attribute::Unparsed(Box::new(attr_item))
}

pub(super) fn build_test_attrs(fixture: AttributeFixture) -> Vec<hir::Attribute> {
    match fixture {
        AttributeFixture::None => Vec::new(),
        AttributeFixture::Allow => {
            vec![hir_attribute_from_segments(PathSegments::new(&["allow"]))]
        }
        AttributeFixture::CfgTest => vec![hir_cfg_test_attribute()],
        AttributeFixture::BuiltInTest => vec![hir_attribute_from_segments(PATH_TEST)],
        AttributeFixture::CustomTest => vec![hir_attribute_from_segments(PathSegments::new(&[
            "my_framework",
            "test",
        ]))],
        AttributeFixture::CfgAndBuiltInTest => {
            vec![
                hir_cfg_test_attribute(),
                hir_attribute_from_segments(PATH_TEST),
            ]
        }
    }
}

pub(super) fn build_additional_test_attributes(include_custom: bool) -> Vec<AttributePath> {
    if include_custom {
        vec![AttributePath::from("my_framework::test")]
    } else {
        Vec::new()
    }
}
