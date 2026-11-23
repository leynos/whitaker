//! Maps rustc attributes onto the documentation-relevant shapes.
//!
//! Classification is by attribute shape, not by resolved value, because before
//! expansion a `#[doc = include_str!("...")]` value is an unexpanded macro
//! call. Asking for the attribute's string value would report that function as
//! undocumented and then diagnose it, which the contract forbids.

use rustc_ast::ast::{AttrArgs, AttrItem, AttrKind, Attribute, MetaItemInner, MetaItemKind};
use rustc_span::sym;

use crate::policy::DocAttributeShape;

/// Classifies one function attribute.
pub(super) fn doc_attribute_shape(attribute: &Attribute) -> DocAttributeShape {
    match &attribute.kind {
        AttrKind::DocComment(..) => DocAttributeShape::DocComment,
        AttrKind::Normal(normal) => normal_attribute_shape(&normal.item),
    }
}

/// Classifies a `#[...]` attribute by its path and argument shape.
fn normal_attribute_shape(item: &AttrItem) -> DocAttributeShape {
    if item.path == sym::doc {
        return doc_attribute_args_shape(item.args.unparsed_ref());
    }
    if item.path == sym::cfg_attr && carries_doc_name_value(item) {
        return DocAttributeShape::CfgAttrDoc;
    }
    DocAttributeShape::Other
}

/// Classifies a `doc` attribute by the shape of its arguments.
const fn doc_attribute_args_shape(args: Option<&AttrArgs>) -> DocAttributeShape {
    match args {
        // `#[doc = "..."]` and `#[doc = include_str!("...")]`; the value is an
        // unexpanded expression, so its form is never inspected.
        Some(AttrArgs::Eq { .. }) => DocAttributeShape::DocNameValue,
        // `#[doc(hidden)]`, `#[doc(alias = "...")]`, and the bare `#[doc]`.
        Some(AttrArgs::Delimited(..) | AttrArgs::Empty) => DocAttributeShape::DocList,
        // An early-parsed `cfg_attr` is produced during expansion, after this
        // pass has run; treat anything unexpected as non-documentation rather
        // than guessing.
        None => DocAttributeShape::Other,
    }
}

/// Reports whether a `cfg_attr` list carries a `doc = ...` entry.
///
/// The first entry is the configuration predicate; only the attributes after it
/// decide whether the conditional attribute would add documentation.
fn carries_doc_name_value(item: &AttrItem) -> bool {
    item.meta_item_list().is_some_and(|entries| {
        entries.iter().skip(1).any(|entry| match entry {
            MetaItemInner::MetaItem(meta) => {
                meta.path == sym::doc && matches!(meta.kind, MetaItemKind::NameValue(_))
            }
            MetaItemInner::Lit(_) => false,
        })
    })
}
