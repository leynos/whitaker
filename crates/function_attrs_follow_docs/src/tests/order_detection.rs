//! Behaviour-driven tests for the attribute ordering detector.
//!
//! These scenarios exercise `detect_misordered_doc` to ensure doc comments
//! continue to precede other outer attributes across common layouts.

use super::{AttrInfo, OrderedAttribute, attribute_within_item, detect_misordered_doc};
use rstest::fixture;
use rstest::rstest;
use rstest_bdd_macros::{given, scenario, then, when};
use rustc_span::{BytePos, DUMMY_SP, Span};
use std::cell::RefCell;
use whitaker_common::attributes::{Attribute, AttributeKind, AttributePath};

impl OrderedAttribute for Attribute {
    fn is_outer(&self) -> bool {
        self.is_outer()
    }

    fn is_doc(&self) -> bool {
        self.is_doc()
    }

    fn span(&self) -> Span {
        DUMMY_SP
    }
}

#[derive(Default)]
struct AttributeWorld {
    attributes: RefCell<Vec<Attribute>>,
}

impl AttributeWorld {
    fn push(&self, path: &str, kind: AttributeKind) {
        self.attributes
            .borrow_mut()
            .push(Attribute::new(AttributePath::from(path), kind));
    }

    fn result(&self) -> Option<(usize, usize)> {
        detect_misordered_doc(self.attributes.borrow().as_slice())
    }
}

#[fixture]
fn world() -> AttributeWorld {
    AttributeWorld::default()
}

#[fixture]
fn result() -> Option<(usize, usize)> {
    None
}

#[given("a doc comment before other attributes")]
fn doc_precedes(world: &AttributeWorld) {
    world.push("doc", AttributeKind::Outer);
    world.push("inline", AttributeKind::Outer);
}

#[given("a doc comment after an attribute")]
fn doc_follows(world: &AttributeWorld) {
    world.push("inline", AttributeKind::Outer);
    world.push("doc", AttributeKind::Outer);
}

#[given("attributes without doc comments")]
fn no_doc(world: &AttributeWorld) {
    world.push("inline", AttributeKind::Outer);
    world.push("allow", AttributeKind::Outer);
}

#[given("a doc comment after an inner attribute")]
fn doc_after_inner(world: &AttributeWorld) {
    world.push("test", AttributeKind::Inner);
    world.push("doc", AttributeKind::Outer);
    world.push("inline", AttributeKind::Outer);
}

#[when("I evaluate the attribute order")]
fn evaluate(world: &AttributeWorld) -> Option<(usize, usize)> {
    world.result()
}

#[then("the order is accepted")]
fn order_ok(result: &Option<(usize, usize)>) {
    assert!(result.is_none());
}

#[then("the order is rejected")]
fn order_rejected(result: &Option<(usize, usize)>) {
    assert!(result.is_some());
}

fn test_span(lo: u32, hi: u32) -> Span {
    Span::with_root_ctxt(BytePos(lo), BytePos(hi))
}

#[rstest]
fn recovered_user_span_drives_source_ordering() {
    let original = test_span(100, 110);
    let recovered = test_span(10, 20);
    let info = AttrInfo {
        span: original,
        user_editable_span: Some(recovered),
        is_doc: false,
        is_outer: true,
    };

    assert_eq!(info.source_order_key(), (recovered.lo(), recovered.hi()));
}

#[rstest]
fn macro_only_attributes_are_dropped_from_item_comparison() {
    let item_span = test_span(10, 40);

    assert!(!attribute_within_item(None, Some(item_span), item_span));
}

#[rstest]
fn recovered_attribute_spans_stay_in_item_bounds() {
    let item_span = test_span(10, 40);
    let attribute_span = test_span(12, 20);

    assert!(attribute_within_item(
        Some(attribute_span),
        Some(item_span),
        item_span
    ));
}

#[rstest]
fn dummy_item_spans_accept_recovered_attributes() {
    let attribute_span = test_span(12, 20);

    assert!(attribute_within_item(Some(attribute_span), None, DUMMY_SP));
}

#[rstest]
fn raw_item_span_is_used_when_recovery_fails() {
    let raw_item_span = test_span(10, 40);
    let attribute_span = test_span(12, 20);

    assert!(attribute_within_item(
        Some(attribute_span),
        None,
        raw_item_span
    ));
}

#[rstest]
fn recovered_attribute_spans_outside_item_are_rejected() {
    let item_span = test_span(10, 40);
    let attribute_span = test_span(12, 45);

    assert!(!attribute_within_item(
        Some(attribute_span),
        Some(item_span),
        item_span
    ));
}

#[scenario(path = "tests/features/function_doc_order.feature", index = 0)]
fn scenario_accepts_doc_first(world: AttributeWorld, result: Option<(usize, usize)>) {
    let _ = (world, result);
}

#[scenario(path = "tests/features/function_doc_order.feature", index = 1)]
fn scenario_rejects_doc_last(world: AttributeWorld, result: Option<(usize, usize)>) {
    let _ = (world, result);
}

#[scenario(path = "tests/features/function_doc_order.feature", index = 2)]
fn scenario_handles_no_doc(world: AttributeWorld, result: Option<(usize, usize)>) {
    let _ = (world, result);
}

#[scenario(path = "tests/features/function_doc_order.feature", index = 3)]
fn scenario_ignores_inner_attributes(world: AttributeWorld, result: Option<(usize, usize)>) {
    let _ = (world, result);
}
