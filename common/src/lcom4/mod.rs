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
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use common::lcom4::MethodInfo;
    ///
    /// let m = MethodInfo::new(
    ///     "process",
    ///     BTreeSet::from(["data".into()]),
    ///     BTreeSet::from(["validate".into()]),
    /// );
    /// assert_eq!(m.name(), "process");
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use common::lcom4::MethodInfo;
    ///
    /// let m = MethodInfo::new("read", BTreeSet::new(), BTreeSet::new());
    /// assert_eq!(m.name(), "read");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the set of field names accessed by this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use common::lcom4::MethodInfo;
    ///
    /// let m = MethodInfo::new(
    ///     "read",
    ///     BTreeSet::from(["buf".into()]),
    ///     BTreeSet::new(),
    /// );
    /// assert!(m.accessed_fields().contains("buf"));
    /// ```
    #[must_use]
    pub fn accessed_fields(&self) -> &BTreeSet<String> {
        &self.accessed_fields
    }

    /// Returns the set of method names called directly by this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use common::lcom4::MethodInfo;
    ///
    /// let m = MethodInfo::new(
    ///     "process",
    ///     BTreeSet::new(),
    ///     BTreeSet::from(["validate".into()]),
    /// );
    /// assert!(m.called_methods().contains("validate"));
    /// ```
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

    /// Returns `true` when the first root has strictly lower rank.
    fn lower_rank(&self, root_a: usize, root_b: usize) -> bool {
        self.rank[root_a] < self.rank[root_b]
    }

    fn union(&mut self, x: usize, y: usize) {
        let root_x = self.find(x);
        let root_y = self.find(y);
        if root_x == root_y {
            return;
        }
        if self.lower_rank(root_x, root_y) {
            self.parent[root_x] = root_y;
        } else if self.lower_rank(root_y, root_x) {
            self.parent[root_y] = root_x;
        } else {
            self.parent[root_y] = root_x;
            self.rank[root_x] += 1;
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

/// Builds an index mapping each field name to the methods that access it.
fn build_field_index<'a>(methods: &'a [MethodInfo]) -> HashMap<&'a str, Vec<usize>> {
    let mut index: HashMap<&'a str, Vec<usize>> = HashMap::new();
    for (idx, method) in methods.iter().enumerate() {
        for field in method.accessed_fields() {
            index.entry(field.as_str()).or_default().push(idx);
        }
    }
    index
}

/// Unions all methods in the given index list with the first method.
fn union_methods_by_index(indices: &[usize], uf: &mut UnionFind) {
    if let Some((&first, rest)) = indices.split_first() {
        for &other in rest {
            uf.union(first, other);
        }
    }
}

/// Builds a field-to-method index and unions methods that share fields.
fn union_by_shared_fields(methods: &[MethodInfo], uf: &mut UnionFind) {
    let field_index = build_field_index(methods);
    for indices in field_index.values() {
        union_methods_by_index(indices, uf);
    }
}

/// Builds a method-name-to-indices map, preserving duplicate names.
fn build_method_index<'a>(methods: &'a [MethodInfo]) -> HashMap<&'a str, Vec<usize>> {
    let mut index: HashMap<&'a str, Vec<usize>> = HashMap::new();
    for (idx, method) in methods.iter().enumerate() {
        index.entry(method.name()).or_default().push(idx);
    }
    index
}

/// Unions caller/callee pairs using the method-name index.
///
/// When multiple methods share a name (e.g. trait impl methods on the
/// same type), the caller is unioned with every matching callee.
/// Calls to names not present in the input are silently ignored.
fn union_by_method_calls(methods: &[MethodInfo], uf: &mut UnionFind) {
    let method_index = build_method_index(methods);

    for (caller_idx, method) in methods.iter().enumerate() {
        let callee_indices: Vec<usize> = method
            .called_methods()
            .iter()
            .filter_map(|name| method_index.get(name.as_str()))
            .flatten()
            .copied()
            .collect();
        for callee_idx in callee_indices {
            uf.union(caller_idx, callee_idx);
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
/// contains the other's name. When multiple methods share a name (e.g.
/// trait impl methods), a call to that name connects the caller with
/// every matching method. Calls to names not present in the input slice
/// are silently ignored.
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

pub mod extract;

pub use extract::MethodInfoBuilder;

#[cfg(test)]
mod tests;
