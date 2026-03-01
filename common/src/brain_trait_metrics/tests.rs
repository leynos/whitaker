//! Unit tests for brain trait metric collection.

use super::*;
use rstest::rstest;

fn mixed_items() -> Vec<TraitItemMetrics> {
    vec![
        TraitItemMetrics::required_method("parse"),
        TraitItemMetrics::default_method("render", 12),
        TraitItemMetrics::associated_type("Output"),
        TraitItemMetrics::associated_const("VERSION"),
    ]
}

fn assert_trait_item(item: &TraitItemMetrics, name: &str, kind: TraitItemKind, cc: Option<usize>) {
    assert_eq!(item.name(), name);
    assert_eq!(item.kind(), kind);
    assert_eq!(item.default_method_cc(), cc);
}

#[expect(
    clippy::too_many_arguments,
    reason = "Helper signature is intentionally explicit per refactor requirement"
)]
fn assert_trait_metrics(
    metrics: &TraitMetrics,
    name: &str,
    total: usize,
    required: usize,
    default: usize,
    cc_sum: usize,
    burden: usize,
) {
    assert_eq!(metrics.trait_name(), name);
    assert_eq!(metrics.total_item_count(), total);
    assert_eq!(metrics.required_method_count(), required);
    assert_eq!(metrics.default_method_count(), default);
    assert_eq!(metrics.default_method_cc_sum(), cc_sum);
    assert_eq!(metrics.implementor_burden(), burden);
}

fn assert_empty_item_metrics(items: &[TraitItemMetrics]) {
    assert_eq!(trait_item_count(items), 0);
    assert_eq!(required_method_count(items), 0);
    assert_eq!(default_method_count(items), 0);
    assert_eq!(default_method_cc_sum(items), 0);
}

#[rstest]
fn required_method_constructor_sets_expected_fields() {
    let item = TraitItemMetrics::required_method("parse");

    assert_trait_item(&item, "parse", TraitItemKind::RequiredMethod, None);
    assert!(item.is_required_method());
    assert!(!item.is_default_method());
}

#[rstest]
fn default_method_constructor_sets_expected_fields() {
    let item = TraitItemMetrics::default_method("render", 12);

    assert_trait_item(&item, "render", TraitItemKind::DefaultMethod, Some(12));
    assert!(item.is_default_method());
    assert!(!item.is_required_method());
}

#[rstest]
fn associated_type_constructor_sets_expected_fields() {
    let item = TraitItemMetrics::associated_type("Output");

    assert_trait_item(&item, "Output", TraitItemKind::AssociatedType, None);
}

#[rstest]
fn associated_const_constructor_sets_expected_fields() {
    let item = TraitItemMetrics::associated_const("VERSION");

    assert_trait_item(&item, "VERSION", TraitItemKind::AssociatedConst, None);
}

#[rstest]
fn trait_item_count_returns_total_number_of_items() {
    assert_eq!(trait_item_count(&mixed_items()), 4);
}

#[rstest]
fn required_method_count_excludes_default_and_associated_items() {
    assert_eq!(required_method_count(&mixed_items()), 1);
}

#[rstest]
fn default_method_count_excludes_required_and_associated_items() {
    assert_eq!(default_method_count(&mixed_items()), 1);
}

#[rstest]
fn default_method_cc_sum_aggregates_only_default_methods() {
    let items = vec![
        TraitItemMetrics::required_method("parse"),
        TraitItemMetrics::default_method("render", 12),
        TraitItemMetrics::default_method("serialise", 8),
        TraitItemMetrics::associated_type("Output"),
    ];

    assert_eq!(default_method_cc_sum(&items), 20);
}

#[rstest]
fn default_method_cc_sum_is_zero_without_default_methods() {
    let items = vec![
        TraitItemMetrics::required_method("parse"),
        TraitItemMetrics::associated_type("Output"),
        TraitItemMetrics::associated_const("VERSION"),
    ];

    assert_eq!(default_method_cc_sum(&items), 0);
}

#[rstest]
fn builder_starts_empty() {
    let builder = TraitMetricsBuilder::new("Parser");
    assert!(builder.is_empty());
}

#[rstest]
fn builder_builds_mixed_trait_metrics() {
    let mut builder = TraitMetricsBuilder::new("Parser");
    builder.add_required_method("parse");
    builder.add_default_method("render", 12, false);
    builder.add_associated_type("Output");
    builder.add_associated_const("VERSION");

    let metrics = builder.build();

    assert_trait_metrics(&metrics, "Parser", 4, 1, 1, 12, 1);
}

#[rstest]
fn builder_add_item_supports_prebuilt_entries() {
    let mut builder = TraitMetricsBuilder::new("Renderer");
    builder.add_item(TraitItemMetrics::required_method("render"));
    builder.add_item(TraitItemMetrics::default_method("validate", 9));

    let metrics = builder.build();

    assert_trait_metrics(&metrics, "Renderer", 2, 1, 1, 9, 1);
}

#[rstest]
fn builder_filters_macro_expanded_default_methods() {
    let mut builder = TraitMetricsBuilder::new("Parser");
    builder.add_required_method("parse");
    builder.add_default_method("generated_helper", 30, true);
    builder.add_default_method("real_default", 12, false);

    let metrics = builder.build();

    assert_trait_metrics(&metrics, "Parser", 2, 1, 1, 12, 1);
}

#[rstest]
fn implementor_burden_equals_required_method_count() {
    let mut builder = TraitMetricsBuilder::new("Transformer");
    builder.add_required_method("parse");
    builder.add_required_method("validate");
    builder.add_default_method("normalise", 7, false);
    builder.add_associated_type("Output");

    let metrics = builder.build();

    assert_eq!(metrics.required_method_count(), 2);
    assert_eq!(metrics.implementor_burden(), 2);
}

#[rstest]
fn empty_trait_has_zeroed_metrics() {
    let metrics = TraitMetricsBuilder::new("EmptyTrait").build();

    assert_trait_metrics(&metrics, "EmptyTrait", 0, 0, 0, 0, 0);
}

#[rstest]
fn trait_item_helpers_handle_empty_input() {
    let items = Vec::<TraitItemMetrics>::new();

    assert_empty_item_metrics(&items);
}
