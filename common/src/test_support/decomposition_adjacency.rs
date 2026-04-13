//! Observable adjacency-construction seams for decomposition advice tests.

use crate::decomposition_advice::community::{SimilarityEdge, build_adjacency};
use thiserror::Error;

/// Declarative edge input for test scenarios.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::decomposition::EdgeInput;
///
/// let edge = EdgeInput { left: 0, right: 1, weight: 10 };
/// assert_eq!(edge.left, 0);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct EdgeInput {
    /// Left endpoint node index in canonical order.
    ///
    /// Callers must provide `left < right`.
    pub left: usize,
    /// Right endpoint node index in canonical order.
    ///
    /// Callers must provide `left < right`.
    pub right: usize,
    /// Positive edge weight carried into the adjacency list.
    pub weight: u64,
}

/// Structured validation errors for declarative adjacency test input.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdjacencyError {
    /// The edge endpoints are not in canonical order.
    #[error("edge {index}: left ({left}) must be less than right ({right})")]
    NonCanonicalEdge {
        /// Edge index within the declarative input slice.
        index: usize,
        /// Left endpoint supplied by the caller.
        left: usize,
        /// Right endpoint supplied by the caller.
        right: usize,
    },
    /// The right endpoint falls outside the declared node range.
    #[error("edge {index}: right ({right}) is out of range for node_count {node_count}")]
    EndpointOutOfRange {
        /// Edge index within the declarative input slice.
        index: usize,
        /// Out-of-range right endpoint.
        right: usize,
        /// Declared node count for the report.
        node_count: usize,
    },
    /// The edge weight violates the positive-weight production contract.
    #[error("edge {index}: weight must be positive")]
    ZeroWeight {
        /// Edge index within the declarative input slice.
        index: usize,
    },
}

/// Observable adjacency-construction results for a set of edges.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::decomposition::{EdgeInput, adjacency_report};
///
/// let report = adjacency_report(3, &[
///     EdgeInput { left: 0, right: 1, weight: 5 },
/// ]);
/// assert!(report.is_ok());
/// let report = report.unwrap();
/// assert_eq!(report.node_count(), 3);
/// assert!(report.is_symmetric());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjacencyReport {
    node_count: usize,
    neighbours: Vec<Vec<(usize, u64)>>,
}

impl AdjacencyReport {
    /// Returns the number of nodes in the adjacency graph.
    ///
    /// ```rust
    /// use whitaker_common::test_support::decomposition::{EdgeInput, adjacency_report};
    ///
    /// let report = adjacency_report(4, &[]).expect("valid input");
    /// assert_eq!(report.node_count(), 4);
    /// ```
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Returns the neighbour list for `node`, sorted by neighbour index.
    ///
    /// Returns `None` if `node` is out of range.
    ///
    /// ```rust
    /// use whitaker_common::test_support::decomposition::{EdgeInput, adjacency_report};
    ///
    /// let report = adjacency_report(3, &[
    ///     EdgeInput { left: 0, right: 2, weight: 7 },
    /// ]).expect("valid input");
    /// assert_eq!(report.neighbours_of(0), Some(&[(2, 7)][..]));
    /// assert_eq!(report.neighbours_of(10), None);
    /// ```
    #[must_use]
    pub fn neighbours_of(&self, node: usize) -> Option<&[(usize, u64)]> {
        self.neighbours.get(node).map(Vec::as_slice)
    }

    /// Returns `true` if all neighbour indices are within bounds.
    ///
    /// ```rust
    /// use whitaker_common::test_support::decomposition::{EdgeInput, adjacency_report};
    ///
    /// let report = adjacency_report(3, &[
    ///     EdgeInput { left: 0, right: 1, weight: 5 },
    /// ]).expect("valid input");
    /// assert!(report.all_indices_in_bounds());
    /// ```
    #[must_use]
    pub fn all_indices_in_bounds(&self) -> bool {
        self.neighbours.iter().all(|bucket| {
            bucket
                .iter()
                .all(|&(neighbour, _)| neighbour < self.node_count)
        })
    }

    /// Returns `true` if the adjacency lists are symmetric: for every entry
    /// `(node -> neighbour, weight)`, the mirrored entry exists.
    ///
    /// ```rust
    /// use whitaker_common::test_support::decomposition::{EdgeInput, adjacency_report};
    ///
    /// let report = adjacency_report(3, &[
    ///     EdgeInput { left: 0, right: 2, weight: 7 },
    /// ]).expect("valid input");
    /// assert!(report.is_symmetric());
    /// ```
    #[must_use]
    pub fn is_symmetric(&self) -> bool {
        self.neighbours.iter().enumerate().all(|(node, bucket)| {
            bucket
                .iter()
                .all(|&(neighbour, weight)| has_mirror(&self.neighbours, neighbour, node, weight))
        })
    }

    /// Returns `true` if each per-node neighbour list is sorted by neighbour
    /// index.
    ///
    /// ```rust
    /// use whitaker_common::test_support::decomposition::{EdgeInput, adjacency_report};
    ///
    /// let report = adjacency_report(4, &[
    ///     EdgeInput { left: 0, right: 1, weight: 5 },
    ///     EdgeInput { left: 0, right: 3, weight: 3 },
    /// ]).expect("valid input");
    /// assert!(report.is_sorted());
    /// ```
    #[must_use]
    pub fn is_sorted(&self) -> bool {
        self.neighbours
            .iter()
            .all(|bucket| bucket.windows(2).all(|pair| pair[0].0 <= pair[1].0))
    }
}

#[expect(
    clippy::unnecessary_map_or,
    reason = "Keep the explicit non-panicking fallback requested in review."
)]
fn has_mirror(
    neighbours: &[Vec<(usize, u64)>],
    neighbour: usize,
    node: usize,
    weight: u64,
) -> bool {
    debug_assert!(
        neighbour < neighbours.len(),
        "has_mirror: neighbour index out of bounds - callers (e.g. adjacency_report) must guarantee valid indices"
    );
    neighbours.get(neighbour).map_or(false, |list| {
        list.iter()
            .any(|&(mirror, mirror_weight)| mirror == node && mirror_weight == weight)
    })
}

/// Builds an [`AdjacencyReport`] from declarative edge input.
///
/// Returns `Err` if any edge endpoint is out of range for the given
/// `node_count` or if `left >= right` (violating the production
/// `build_similarity_edges` contract).
///
/// # Examples
///
/// ```rust
/// use whitaker_common::test_support::decomposition::{EdgeInput, adjacency_report};
///
/// let report = adjacency_report(3, &[
///     EdgeInput { left: 0, right: 2, weight: 7 },
/// ]).expect("valid input");
/// assert_eq!(report.node_count(), 3);
/// ```
///
/// # Errors
///
/// Returns a typed validation error when an edge violates the production
/// input contract.
pub fn adjacency_report(
    node_count: usize,
    edges: &[EdgeInput],
) -> Result<AdjacencyReport, AdjacencyError> {
    let similarity_edges = validate_edges(node_count, edges)?;
    let neighbours = build_adjacency(node_count, &similarity_edges);

    Ok(AdjacencyReport {
        node_count,
        neighbours,
    })
}

fn validate_edges(
    node_count: usize,
    edges: &[EdgeInput],
) -> Result<Vec<SimilarityEdge>, AdjacencyError> {
    let mut result = Vec::with_capacity(edges.len());

    for (index, edge) in edges.iter().enumerate() {
        if edge.left >= edge.right {
            return Err(AdjacencyError::NonCanonicalEdge {
                index,
                left: edge.left,
                right: edge.right,
            });
        }
        if edge.right >= node_count {
            return Err(AdjacencyError::EndpointOutOfRange {
                index,
                right: edge.right,
                node_count,
            });
        }
        if edge.weight == 0 {
            return Err(AdjacencyError::ZeroWeight { index });
        }
        result.push(SimilarityEdge::new(edge.left, edge.right, edge.weight));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{AdjacencyError, EdgeInput, adjacency_report};

    #[test]
    fn adjacency_report_rejects_non_canonical_edges_with_structured_error() {
        let result = adjacency_report(
            3,
            &[EdgeInput {
                left: 1,
                right: 1,
                weight: 7,
            }],
        );

        assert_eq!(
            result,
            Err(AdjacencyError::NonCanonicalEdge {
                index: 0,
                left: 1,
                right: 1,
            })
        );
    }

    #[test]
    fn adjacency_report_rejects_out_of_range_endpoints_with_structured_error() {
        let result = adjacency_report(
            3,
            &[EdgeInput {
                left: 1,
                right: 3,
                weight: 7,
            }],
        );

        assert_eq!(
            result,
            Err(AdjacencyError::EndpointOutOfRange {
                index: 0,
                right: 3,
                node_count: 3,
            })
        );
    }

    #[test]
    fn adjacency_report_rejects_zero_weight_with_structured_error() {
        let result = adjacency_report(
            3,
            &[EdgeInput {
                left: 0,
                right: 2,
                weight: 0,
            }],
        );

        assert_eq!(result, Err(AdjacencyError::ZeroWeight { index: 0 }));
    }
}
