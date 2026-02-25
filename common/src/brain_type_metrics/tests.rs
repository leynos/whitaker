//! Unit tests for brain type metric collection.

use super::*;
use rstest::rstest;

// ---------------------------------------------------------------------------
// MethodMetrics
// ---------------------------------------------------------------------------

#[rstest]
fn construction_and_accessors() {
    let m = MethodMetrics::new("parse", 31, 140);
    assert_eq!(m.name(), "parse");
    assert_eq!(m.cognitive_complexity(), 31);
    assert_eq!(m.lines_of_code(), 140);
}

#[rstest]
#[case(("parse", 30, 100, 25, 80, true))]
#[case(("parse", 25, 80, 25, 80, true))]
#[case(("helper", 20, 100, 25, 80, false))]
#[case(("complex_but_short", 30, 40, 25, 80, false))]
#[case(("tiny", 5, 20, 25, 80, false))]
#[case(("any", 0, 0, 0, 0, true))]
fn is_brain_method_threshold_cases(
    #[case] (name, cc, loc, cc_threshold, loc_threshold, expected): (
        &str,
        usize,
        usize,
        usize,
        usize,
        bool,
    ),
) {
    let m = MethodMetrics::new(name, cc, loc);
    assert_eq!(m.is_brain_method(cc_threshold, loc_threshold), expected);
}

// ---------------------------------------------------------------------------
// weighted_methods_count
// ---------------------------------------------------------------------------

#[rstest]
fn wmc_empty_slice_returns_zero() {
    assert_eq!(weighted_methods_count(&[]), 0);
}

#[rstest]
fn wmc_single_method_returns_its_cc() {
    let methods = vec![MethodMetrics::new("a", 10, 50)];
    assert_eq!(weighted_methods_count(&methods), 10);
}

#[rstest]
fn wmc_multiple_methods_returns_sum() {
    let methods = vec![
        MethodMetrics::new("a", 10, 50),
        MethodMetrics::new("b", 20, 60),
        MethodMetrics::new("c", 5, 30),
    ];
    assert_eq!(weighted_methods_count(&methods), 35);
}

#[rstest]
fn wmc_methods_with_zero_cc_contribute_nothing() {
    let methods = vec![
        MethodMetrics::new("a", 0, 50),
        MethodMetrics::new("b", 15, 60),
        MethodMetrics::new("c", 0, 10),
    ];
    assert_eq!(weighted_methods_count(&methods), 15);
}

// ---------------------------------------------------------------------------
// brain_methods
// ---------------------------------------------------------------------------

#[rstest]
fn brain_methods_empty_slice_returns_empty() {
    let result = brain_methods(&[], 25, 80);
    assert!(result.is_empty());
}

#[rstest]
fn brain_methods_no_qualifying_methods() {
    let methods = vec![
        MethodMetrics::new("small", 5, 20),
        MethodMetrics::new("medium", 15, 60),
    ];
    let result = brain_methods(&methods, 25, 80);
    assert!(result.is_empty());
}

#[rstest]
fn brain_methods_one_qualifying_method() {
    let methods = vec![
        MethodMetrics::new("parse", 30, 100),
        MethodMetrics::new("helper", 5, 20),
    ];
    let result = brain_methods(&methods, 25, 80);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name(), "parse");
}

#[rstest]
fn brain_methods_multiple_qualifying_in_order() {
    let methods = vec![
        MethodMetrics::new("alpha", 30, 100),
        MethodMetrics::new("beta", 5, 20),
        MethodMetrics::new("gamma", 40, 200),
    ];
    let result = brain_methods(&methods, 25, 80);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].name(), "alpha");
    assert_eq!(result[1].name(), "gamma");
}

#[rstest]
#[case("complex_short", 30, 40)]
#[case("simple_long", 10, 100)]
fn brain_methods_single_threshold_match_excluded(
    #[case] method_name: &str,
    #[case] cc: usize,
    #[case] loc: usize,
) {
    let methods = vec![MethodMetrics::new(method_name, cc, loc)];
    let result = brain_methods(&methods, 25, 80);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// ForeignReferenceSet
// ---------------------------------------------------------------------------

#[rstest]
fn foreign_set_empty_has_zero_count() {
    let refs = ForeignReferenceSet::new();
    assert_eq!(refs.count(), 0);
    assert!(refs.is_empty());
}

#[rstest]
fn foreign_set_distinct_references_counted() {
    let mut refs = ForeignReferenceSet::new();
    refs.record_reference("std::collections", false);
    refs.record_reference("serde::Deserialize", false);
    assert_eq!(refs.count(), 2);
    assert!(!refs.is_empty());
}

#[rstest]
fn foreign_set_duplicate_references_deduplicated() {
    let mut refs = ForeignReferenceSet::new();
    refs.record_reference("std::io", false);
    refs.record_reference("std::io", false);
    refs.record_reference("std::io", false);
    assert_eq!(refs.count(), 1);
}

#[rstest]
fn foreign_set_macro_expanded_filtered() {
    let mut refs = ForeignReferenceSet::new();
    refs.record_reference("macro_gen::Type", true);
    assert_eq!(refs.count(), 0);
    assert!(refs.is_empty());
}

#[rstest]
fn foreign_set_mixed_expanded_and_regular() {
    let mut refs = ForeignReferenceSet::new();
    refs.record_reference("std::fmt", true);
    refs.record_reference("serde::Serialize", false);
    refs.record_reference("tokio::fs", false);
    refs.record_reference("macro_gen::Util", true);
    assert_eq!(refs.count(), 2);
    assert!(refs.references().contains("serde::Serialize"));
    assert!(refs.references().contains("tokio::fs"));
}

#[rstest]
fn foreign_set_all_from_expansion_yields_empty() {
    let mut refs = ForeignReferenceSet::new();
    refs.record_reference("a", true);
    refs.record_reference("b", true);
    assert!(refs.is_empty());
}

#[rstest]
fn foreign_reach_count_convenience() {
    let refs = vec![
        ("std::io".into(), false),
        ("std::io".into(), false),
        ("serde::de".into(), false),
        ("macro_gen".into(), true),
    ];
    assert_eq!(foreign_reach_count(refs), 2);
}

#[rstest]
fn foreign_reach_count_empty_iterator() {
    assert_eq!(foreign_reach_count(Vec::new()), 0);
}

// ---------------------------------------------------------------------------
// TypeMetricsBuilder
// ---------------------------------------------------------------------------

#[rstest]
fn builder_empty_produces_zero_metrics() {
    let builder = TypeMetricsBuilder::new("Empty", 25, 80);
    let metrics = builder.build();
    assert_eq!(metrics.type_name(), "Empty");
    assert_eq!(metrics.wmc(), 0);
    assert!(metrics.brain_method_names().is_empty());
    assert_eq!(metrics.brain_method_count(), 0);
    assert_eq!(metrics.lcom4(), 0);
    assert_eq!(metrics.foreign_reach(), 0);
    assert_eq!(metrics.method_count(), 0);
}

#[rstest]
fn builder_computes_wmc() {
    let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
    builder.add_method("a", 10, 50);
    builder.add_method("b", 20, 60);
    let metrics = builder.build();
    assert_eq!(metrics.wmc(), 30);
}

#[rstest]
fn builder_identifies_brain_methods() {
    let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
    builder.add_method("parse", 31, 140);
    builder.add_method("helper", 5, 20);
    builder.add_method("transform", 28, 90);
    let metrics = builder.build();
    assert_eq!(metrics.brain_method_count(), 2);
    assert_eq!(metrics.brain_method_names(), &["parse", "transform"]);
}

#[rstest]
fn builder_defaults_lcom4_and_foreign_reach() {
    let builder = TypeMetricsBuilder::new("Foo", 25, 80);
    let metrics = builder.build();
    assert_eq!(metrics.lcom4(), 0);
    assert_eq!(metrics.foreign_reach(), 0);
}

#[rstest]
fn builder_preserves_set_values() {
    let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
    builder.set_lcom4(3);
    builder.set_foreign_reach(7);
    let metrics = builder.build();
    assert_eq!(metrics.lcom4(), 3);
    assert_eq!(metrics.foreign_reach(), 7);
}

#[rstest]
fn builder_method_count_is_correct() {
    let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
    builder.add_method("a", 1, 10);
    builder.add_method("b", 2, 20);
    builder.add_method("c", 3, 30);
    let metrics = builder.build();
    assert_eq!(metrics.method_count(), 3);
}

// ---------------------------------------------------------------------------
// TypeMetrics accessors via builder
// ---------------------------------------------------------------------------

#[rstest]
fn type_metrics_all_accessors() {
    let mut builder = TypeMetricsBuilder::new("Bar", 25, 80);
    builder.add_method("parse", 60, 140);
    builder.add_method("render", 58, 120);
    builder.add_method("tiny", 2, 5);
    builder.set_lcom4(3);
    builder.set_foreign_reach(5);
    let tm = builder.build();
    assert_eq!(tm.type_name(), "Bar");
    assert_eq!(tm.wmc(), 120);
    assert_eq!(tm.brain_method_names(), &["parse", "render"]);
    assert_eq!(tm.brain_method_count(), 2);
    assert_eq!(tm.lcom4(), 3);
    assert_eq!(tm.foreign_reach(), 5);
    assert_eq!(tm.method_count(), 3);
}
