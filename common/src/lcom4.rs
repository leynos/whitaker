//! LCOM4 cohesion analysis for method relationship graphs.
//!
//! LCOM4 (Lack of Cohesion in Methods, version 4) measures type cohesion by
//! modelling each method as a node in an undirected graph. Edges connect
//! methods that share a field access or where one method directly calls
//! another on the same type. The metric equals the number of connected
//! components: LCOM4 == 1 indicates high cohesion, while LCOM4 >= 2
//! suggests the type bundles unrelated responsibilities.
//!
//! This module provides a pure library helper that operates on pre-extracted
//! method metadata (`MethodInfo`). It does not depend on `rustc_private` or
//! any HIR types — the HIR traversal that populates `MethodInfo` is handled
//! by individual lint drivers.
//!
//! See `docs/brain-trust-lints-design.md` §Cohesion analysis (LCOM4) for the
//! full design rationale.

use std::collections::{BTreeSet, HashMap};

/// Metadata for a single method, used to build the LCOM4 method graph.
///
/// Each method carries its name, the set of fields it accesses, and the set
/// of other methods on the same type that it calls directly. Field names and
/// method names are plain strings extracted by the caller from HIR or other
/// analysis passes.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeSet;
/// use common::lcom4::MethodInfo;
///
/// let method = MethodInfo::new(
///     "process",
///     BTreeSet::from(["data".into(), "buffer".into()]),
///     BTreeSet::new(),
/// );
///
/// assert_eq!(method.name(), "process");
/// assert!(method.accessed_fields().contains("data"));
/// assert!(method.called_methods().is_empty());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodInfo {
    name: String,
    accessed_fields: BTreeSet<String>,
    called_methods: BTreeSet<String>,
}

impl MethodInfo {
    /// Creates a new `MethodInfo` with the given name, accessed fields, and
    /// called methods.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        accessed_fields: BTreeSet<String>,
        called_methods: BTreeSet<String>,
    ) -> Self {
        Self {
            name: name.into(),
            accessed_fields,
            called_methods,
        }
    }

    /// Returns the method name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the set of field names accessed by this method.
    #[must_use]
    pub fn accessed_fields(&self) -> &BTreeSet<String> {
        &self.accessed_fields
    }

    /// Returns the set of method names called directly by this method.
    #[must_use]
    pub fn called_methods(&self) -> &BTreeSet<String> {
        &self.called_methods
    }
}

/// Disjoint-set forest for connected component counting.
///
/// Uses path compression and union-by-rank for near-constant amortized
/// operations. This is an internal implementation detail of
/// [`cohesion_components`] and is not exposed publicly.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let root_x = self.find(x);
        let root_y = self.find(y);
        if root_x == root_y {
            return;
        }
        match self.rank[root_x].cmp(&self.rank[root_y]) {
            std::cmp::Ordering::Less => self.parent[root_x] = root_y,
            std::cmp::Ordering::Greater => self.parent[root_y] = root_x,
            std::cmp::Ordering::Equal => {
                self.parent[root_y] = root_x;
                self.rank[root_x] += 1;
            }
        }
    }

    fn component_count(&mut self) -> usize {
        let n = self.parent.len();
        // Flatten the forest so every node points directly to its root.
        for i in 0..n {
            self.find(i);
        }
        let mut roots: Vec<usize> = self.parent[..n].to_vec();
        roots.sort_unstable();
        roots.dedup();
        roots.len()
    }
}

/// Builds a field-to-method index and unions methods that share fields.
fn union_by_shared_fields(methods: &[MethodInfo], uf: &mut UnionFind) {
    let mut field_index: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, method) in methods.iter().enumerate() {
        for field in method.accessed_fields() {
            field_index.entry(field.as_str()).or_default().push(idx);
        }
    }
    for indices in field_index.values() {
        // Union all methods that share this field with the first one.
        if let Some((&first, rest)) = indices.split_first() {
            for &other in rest {
                uf.union(first, other);
            }
        }
    }
}

/// Builds a method-name-to-index map and unions caller/callee pairs.
fn union_by_method_calls(methods: &[MethodInfo], uf: &mut UnionFind) {
    let method_index: HashMap<&str, usize> = methods
        .iter()
        .enumerate()
        .map(|(idx, m)| (m.name(), idx))
        .collect();

    for (caller_idx, method) in methods.iter().enumerate() {
        for callee_name in method.called_methods() {
            if let Some(&callee_idx) = method_index.get(callee_name.as_str()) {
                uf.union(caller_idx, callee_idx);
            }
            // Calls to methods not present in the input are silently ignored.
        }
    }
}

/// Counts connected components in the method relationship graph (LCOM4).
///
/// Returns `0` for an empty method slice, `1` when all methods form a
/// single cohesive group, and `n >= 2` when the type contains `n` unrelated
/// method clusters.
///
/// Two methods are connected when they share at least one field name in
/// their accessed-fields sets, or when one method's called-methods set
/// contains the other's name. Calls to methods not present in the input
/// slice are silently ignored.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeSet;
/// use common::lcom4::{MethodInfo, cohesion_components};
///
/// let methods = vec![
///     MethodInfo::new("read", BTreeSet::from(["buf".into()]), BTreeSet::new()),
///     MethodInfo::new("write", BTreeSet::from(["buf".into()]), BTreeSet::new()),
/// ];
///
/// assert_eq!(cohesion_components(&methods), 1);
/// ```
///
/// ```
/// use std::collections::BTreeSet;
/// use common::lcom4::{MethodInfo, cohesion_components};
///
/// let methods = vec![
///     MethodInfo::new("parse", BTreeSet::from(["input".into()]), BTreeSet::new()),
///     MethodInfo::new("render", BTreeSet::from(["output".into()]), BTreeSet::new()),
/// ];
///
/// assert_eq!(cohesion_components(&methods), 2);
/// ```
#[must_use]
pub fn cohesion_components(methods: &[MethodInfo]) -> usize {
    if methods.is_empty() {
        return 0;
    }

    let mut uf = UnionFind::new(methods.len());
    union_by_shared_fields(methods, &mut uf);
    union_by_method_calls(methods, &mut uf);
    uf.component_count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Helper: builds a `MethodInfo` with the given fields and no calls.
    fn method_with_fields(name: &str, fields: &[&str]) -> MethodInfo {
        MethodInfo::new(
            name,
            fields.iter().map(|s| (*s).to_string()).collect(),
            BTreeSet::new(),
        )
    }

    /// Helper: builds a `MethodInfo` with no fields and the given calls.
    fn method_with_calls(name: &str, calls: &[&str]) -> MethodInfo {
        MethodInfo::new(
            name,
            BTreeSet::new(),
            calls.iter().map(|s| (*s).to_string()).collect(),
        )
    }

    /// Helper: builds a `MethodInfo` with both fields and calls.
    fn method_with_fields_and_calls(name: &str, fields: &[&str], calls: &[&str]) -> MethodInfo {
        MethodInfo::new(
            name,
            fields.iter().map(|s| (*s).to_string()).collect(),
            calls.iter().map(|s| (*s).to_string()).collect(),
        )
    }

    // --- Happy paths ---

    #[rstest]
    fn single_method_yields_one_component() {
        let methods = vec![method_with_fields("process", &["data"])];
        assert_eq!(cohesion_components(&methods), 1);
    }

    #[rstest]
    fn two_methods_sharing_field_yields_one_component() {
        let methods = vec![
            method_with_fields("read", &["buffer"]),
            method_with_fields("write", &["buffer"]),
        ];
        assert_eq!(cohesion_components(&methods), 1);
    }

    #[rstest]
    fn two_methods_with_direct_call_yields_one_component() {
        let methods = vec![
            method_with_calls("process", &["validate"]),
            method_with_fields("validate", &[]),
        ];
        assert_eq!(cohesion_components(&methods), 1);
    }

    #[rstest]
    fn transitive_field_sharing_yields_one_component() {
        let methods = vec![
            method_with_fields("a", &["x"]),
            method_with_fields("b", &["x", "y"]),
            method_with_fields("c", &["y"]),
        ];
        assert_eq!(cohesion_components(&methods), 1);
    }

    #[rstest]
    fn all_methods_share_common_field() {
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
    fn two_disjoint_methods_yield_two_components() {
        let methods = vec![
            method_with_fields("parse", &["input"]),
            method_with_fields("render", &["output"]),
        ];
        assert_eq!(cohesion_components(&methods), 2);
    }

    #[rstest]
    fn three_methods_two_clusters() {
        let methods = vec![
            method_with_fields("a", &["x"]),
            method_with_fields("b", &["x"]),
            method_with_fields("c", &["y"]),
        ];
        assert_eq!(cohesion_components(&methods), 2);
    }

    #[rstest]
    fn four_methods_three_clusters() {
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
    fn methods_with_empty_fields_and_no_calls_are_isolated() {
        let methods = vec![
            method_with_fields("alpha", &[]),
            method_with_fields("beta", &[]),
            method_with_fields("gamma", &[]),
        ];
        assert_eq!(cohesion_components(&methods), 3);
    }

    #[rstest]
    fn self_call_does_not_connect_to_others() {
        let methods = vec![
            method_with_calls("a", &["a"]),
            method_with_calls("b", &["b"]),
        ];
        assert_eq!(cohesion_components(&methods), 2);
    }

    #[rstest]
    fn mixed_field_sharing_and_calls() {
        let methods = vec![
            method_with_fields("a", &["x"]),
            method_with_fields("b", &["x"]),
            method_with_calls("c", &["a"]),
        ];
        assert_eq!(cohesion_components(&methods), 1);
    }

    #[rstest]
    fn method_calls_unknown_method() {
        let methods = vec![
            method_with_calls("a", &["nonexistent"]),
            method_with_fields("b", &["y"]),
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

    #[rstest]
    fn bidirectional_call_connects_methods() {
        let methods = vec![
            method_with_calls("a", &["b"]),
            method_with_calls("b", &["a"]),
        ];
        assert_eq!(cohesion_components(&methods), 1);
    }

    #[rstest]
    fn fields_and_calls_combine_to_connect() {
        // a --field:x-- b, c --calls:a--> a, d --field:z-- (isolated)
        let methods = vec![
            method_with_fields("a", &["x"]),
            method_with_fields("b", &["x"]),
            method_with_fields_and_calls("c", &[], &["a"]),
            method_with_fields("d", &["z"]),
        ];
        assert_eq!(cohesion_components(&methods), 2);
    }
}
