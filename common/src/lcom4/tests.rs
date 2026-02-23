//! rstest-based unit tests for [`super::cohesion_components`] and supporting types.

use super::*;
use rstest::rstest;
use std::collections::BTreeSet;

// --- Fixtures (shared setup) ---

#[rstest::fixture]
fn method_with_fields() -> fn(&str, &[&str]) -> MethodInfo {
    |name: &str, fields: &[&str]| -> MethodInfo {
        MethodInfo::new(
            name,
            fields.iter().map(|s| (*s).to_string()).collect(),
            BTreeSet::new(),
        )
    }
}

#[rstest::fixture]
fn method_with_calls() -> fn(&str, &[&str]) -> MethodInfo {
    |name: &str, calls: &[&str]| -> MethodInfo {
        MethodInfo::new(
            name,
            BTreeSet::new(),
            calls.iter().map(|s| (*s).to_string()).collect(),
        )
    }
}

#[rstest::fixture]
fn method_with_fields_and_calls() -> fn(&str, &[&str], &[&str]) -> MethodInfo {
    |name: &str, fields: &[&str], calls: &[&str]| -> MethodInfo {
        MethodInfo::new(
            name,
            fields.iter().map(|s| (*s).to_string()).collect(),
            calls.iter().map(|s| (*s).to_string()).collect(),
        )
    }
}

// --- Field-based cohesion (parameterised) ---

#[rstest]
#[case::single_method(
    "single method yields one component",
    vec![("process", &["data"] as &[&str])],
    1
)]
#[case::shared_field(
    "two methods sharing a field yields one component",
    vec![("read", &["buffer"] as &[&str]), ("write", &["buffer"])],
    1
)]
#[case::transitive_sharing(
    "transitive field sharing yields one component",
    vec![("a", &["x"] as &[&str]), ("b", &["x", "y"]), ("c", &["y"])],
    1
)]
#[case::common_field(
    "all methods share a common field",
    vec![
        ("alpha", &["shared"] as &[&str]),
        ("beta", &["shared"]),
        ("gamma", &["shared"]),
        ("delta", &["shared"]),
    ],
    1
)]
#[case::disjoint(
    "two disjoint methods yield two components",
    vec![("parse", &["input"] as &[&str]), ("render", &["output"])],
    2
)]
#[case::two_clusters(
    "three methods in two clusters",
    vec![("a", &["x"] as &[&str]), ("b", &["x"]), ("c", &["y"])],
    2
)]
#[case::three_clusters(
    "four methods in three clusters",
    vec![("a", &["x"] as &[&str]), ("b", &["x"]), ("c", &["y"]), ("d", &["z"])],
    3
)]
#[case::isolated_no_fields(
    "methods with empty fields and no calls are isolated",
    vec![
        ("alpha", &[] as &[&str]),
        ("beta", &[]),
        ("gamma", &[]),
    ],
    3
)]
fn field_based_cohesion(
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
    #[case] scenario: &str,
    #[case] method_specs: Vec<(&str, &[&str])>,
    #[case] expected: usize,
) {
    let methods: Vec<MethodInfo> = method_specs
        .iter()
        .map(|(name, fields)| method_with_fields(name, fields))
        .collect();
    assert_eq!(cohesion_components(&methods), expected, "{scenario}");
}

// --- Call-based cohesion (parameterised) ---

#[rstest]
#[case::direct_call(
    "two methods with a direct call yields one component",
    vec![("process", &[] as &[&str], &["validate"] as &[&str]), ("validate", &[], &[])],
    1
)]
#[case::self_call(
    "self-call does not connect to others",
    vec![("a", &[] as &[&str], &["a"] as &[&str]), ("b", &[], &["b"])],
    2
)]
#[case::unknown_callee(
    "call to unknown method is silently ignored",
    vec![("a", &[] as &[&str], &["nonexistent"] as &[&str]), ("b", &["y"], &[])],
    2
)]
#[case::bidirectional(
    "bidirectional call connects methods",
    vec![("a", &[] as &[&str], &["b"] as &[&str]), ("b", &[], &["a"])],
    1
)]
fn call_based_cohesion(
    method_with_fields_and_calls: fn(&str, &[&str], &[&str]) -> MethodInfo,
    #[case] scenario: &str,
    #[case] method_specs: Vec<(&str, &[&str], &[&str])>,
    #[case] expected: usize,
) {
    let methods: Vec<MethodInfo> = method_specs
        .iter()
        .map(|(name, fields, calls)| method_with_fields_and_calls(name, fields, calls))
        .collect();
    assert_eq!(cohesion_components(&methods), expected, "{scenario}");
}

// --- Tests with distinct concerns or mixed fixtures ---

#[rstest]
fn empty_methods_yields_zero() {
    assert_eq!(cohesion_components(&[]), 0);
}

#[rstest]
fn mixed_field_sharing_and_calls(
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
    method_with_calls: fn(&str, &[&str]) -> MethodInfo,
) {
    let methods = vec![
        method_with_fields("a", &["x"]),
        method_with_fields("b", &["x"]),
        method_with_calls("c", &["a"]),
    ];
    assert_eq!(cohesion_components(&methods), 1);
}

#[rstest]
fn duplicate_method_names_connected_via_call(
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
    method_with_calls: fn(&str, &[&str]) -> MethodInfo,
) {
    // Two methods named "do_work" (e.g. trait impls) plus a caller.
    // The call to "do_work" should connect the caller to both.
    let methods = vec![
        method_with_fields("do_work", &["x"]),
        method_with_fields("do_work", &["y"]),
        method_with_calls("dispatch", &["do_work"]),
    ];
    assert_eq!(cohesion_components(&methods), 1);
}

#[rstest]
fn fields_and_calls_combine_to_connect(
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
    method_with_fields_and_calls: fn(&str, &[&str], &[&str]) -> MethodInfo,
) {
    // a --field:x-- b, c --calls:a--> a, d --field:z-- (isolated)
    let methods = vec![
        method_with_fields("a", &["x"]),
        method_with_fields("b", &["x"]),
        method_with_fields_and_calls("c", &[], &["a"]),
        method_with_fields("d", &["z"]),
    ];
    assert_eq!(cohesion_components(&methods), 2);
}

// --- Union-find internals ---

#[rstest]
fn union_find_single_element() {
    let mut uf = UnionFind::new(1);
    assert_eq!(uf.component_count(), 1);
}

#[rstest]
fn union_find_merge_reduces_count() {
    let mut uf = UnionFind::new(4);
    assert_eq!(uf.component_count(), 4);
    uf.union(0, 1);
    assert_eq!(uf.component_count(), 3);
    uf.union(2, 3);
    assert_eq!(uf.component_count(), 2);
    uf.union(0, 2);
    assert_eq!(uf.component_count(), 1);
}
