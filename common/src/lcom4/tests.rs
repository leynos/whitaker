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

// --- Happy paths ---

#[rstest]
fn single_method_yields_one_component(method_with_fields: fn(&str, &[&str]) -> MethodInfo) {
    let methods = vec![method_with_fields("process", &["data"])];
    assert_eq!(cohesion_components(&methods), 1);
}

#[rstest]
fn two_methods_sharing_field_yields_one_component(
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
) {
    let methods = vec![
        method_with_fields("read", &["buffer"]),
        method_with_fields("write", &["buffer"]),
    ];
    assert_eq!(cohesion_components(&methods), 1);
}

#[rstest]
fn two_methods_with_direct_call_yields_one_component(
    method_with_calls: fn(&str, &[&str]) -> MethodInfo,
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
) {
    let methods = vec![
        method_with_calls("process", &["validate"]),
        method_with_fields("validate", &[]),
    ];
    assert_eq!(cohesion_components(&methods), 1);
}

#[rstest]
fn transitive_field_sharing_yields_one_component(
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
) {
    let methods = vec![
        method_with_fields("a", &["x"]),
        method_with_fields("b", &["x", "y"]),
        method_with_fields("c", &["y"]),
    ];
    assert_eq!(cohesion_components(&methods), 1);
}

#[rstest]
fn all_methods_share_common_field(method_with_fields: fn(&str, &[&str]) -> MethodInfo) {
    let methods = vec![
        method_with_fields("alpha", &["shared"]),
        method_with_fields("beta", &["shared"]),
        method_with_fields("gamma", &["shared"]),
        method_with_fields("delta", &["shared"]),
    ];
    assert_eq!(cohesion_components(&methods), 1);
}

// --- Unhappy paths ---

#[rstest]
fn two_disjoint_methods_yield_two_components(method_with_fields: fn(&str, &[&str]) -> MethodInfo) {
    let methods = vec![
        method_with_fields("parse", &["input"]),
        method_with_fields("render", &["output"]),
    ];
    assert_eq!(cohesion_components(&methods), 2);
}

#[rstest]
fn three_methods_two_clusters(method_with_fields: fn(&str, &[&str]) -> MethodInfo) {
    let methods = vec![
        method_with_fields("a", &["x"]),
        method_with_fields("b", &["x"]),
        method_with_fields("c", &["y"]),
    ];
    assert_eq!(cohesion_components(&methods), 2);
}

#[rstest]
fn four_methods_three_clusters(method_with_fields: fn(&str, &[&str]) -> MethodInfo) {
    let methods = vec![
        method_with_fields("a", &["x"]),
        method_with_fields("b", &["x"]),
        method_with_fields("c", &["y"]),
        method_with_fields("d", &["z"]),
    ];
    assert_eq!(cohesion_components(&methods), 3);
}

// --- Edge cases ---

#[rstest]
fn empty_methods_yields_zero() {
    assert_eq!(cohesion_components(&[]), 0);
}

#[rstest]
fn methods_with_empty_fields_and_no_calls_are_isolated(
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
) {
    let methods = vec![
        method_with_fields("alpha", &[]),
        method_with_fields("beta", &[]),
        method_with_fields("gamma", &[]),
    ];
    assert_eq!(cohesion_components(&methods), 3);
}

#[rstest]
fn self_call_does_not_connect_to_others(method_with_calls: fn(&str, &[&str]) -> MethodInfo) {
    let methods = vec![
        method_with_calls("a", &["a"]),
        method_with_calls("b", &["b"]),
    ];
    assert_eq!(cohesion_components(&methods), 2);
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
fn method_calls_unknown_method(
    method_with_calls: fn(&str, &[&str]) -> MethodInfo,
    method_with_fields: fn(&str, &[&str]) -> MethodInfo,
) {
    let methods = vec![
        method_with_calls("a", &["nonexistent"]),
        method_with_fields("b", &["y"]),
    ];
    assert_eq!(cohesion_components(&methods), 2);
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

#[rstest]
fn bidirectional_call_connects_methods(method_with_calls: fn(&str, &[&str]) -> MethodInfo) {
    let methods = vec![
        method_with_calls("a", &["b"]),
        method_with_calls("b", &["a"]),
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
