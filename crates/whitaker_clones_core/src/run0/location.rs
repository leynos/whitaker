//! Physical location of a normalized clone fragment.
//!
//! A fragment's identity depends on its normalization profile, but its
//! *physical* location does not: the T1 and T2 fragments of one code region
//! share a file URI and byte span. The AST pass groups evidence by physical
//! location so it never reports the same region twice.

use std::{cmp::Ordering, ops::Range};

/// The file and byte span of a clone fragment.
///
/// Ordering is `(file_uri, start, end)`, which gives the canonical order of a
/// physical pair and keeps Run 1 output deterministic.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::FragmentLocation;
///
/// let location = FragmentLocation::new("src/lib.rs", 0..12);
/// assert_eq!(location.file_uri(), "src/lib.rs");
/// assert_eq!(location.span(), 0..12);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FragmentLocation {
    file_uri: String,
    span: Range<usize>,
}

impl FragmentLocation {
    /// Creates a physical fragment location.
    #[must_use]
    pub fn new(file_uri: impl Into<String>, span: Range<usize>) -> Self {
        Self {
            file_uri: file_uri.into(),
            span,
        }
    }

    /// Returns the source artefact URI.
    #[must_use]
    pub const fn file_uri(&self) -> &str { self.file_uri.as_str() }

    /// Returns the half-open byte span.
    #[must_use]
    pub fn span(&self) -> Range<usize> { self.span.clone() }

    /// Returns the span start offset.
    #[must_use]
    pub const fn start(&self) -> usize { self.span.start }

    /// Returns the exclusive span end offset.
    #[must_use]
    pub const fn end(&self) -> usize { self.span.end }

    /// Returns the span width in bytes.
    #[must_use]
    pub const fn len(&self) -> usize { self.span.end.saturating_sub(self.span.start) }

    /// Reports whether the span covers no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool { self.len() == 0 }
}

impl Ord for FragmentLocation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.file_uri
            .cmp(&other.file_uri)
            .then_with(|| self.start().cmp(&other.start()))
            .then_with(|| self.end().cmp(&other.end()))
    }
}

impl PartialOrd for FragmentLocation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

#[cfg(test)]
mod tests {
    //! Ordering and accessor tests for physical fragment locations.

    use super::*;

    /// Check that the canonical order sorts by file, then start, then end.
    #[test]
    fn ordering_is_file_then_span() {
        let mut locations = [
            FragmentLocation::new("src/b.rs", 0..4),
            FragmentLocation::new("src/a.rs", 10..20),
            FragmentLocation::new("src/a.rs", 10..14),
        ];
        locations.sort();

        let order = locations
            .iter()
            .map(|location| (location.file_uri(), location.start(), location.end()))
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            vec![
                ("src/a.rs", 10, 14),
                ("src/a.rs", 10, 20),
                ("src/b.rs", 0, 4)
            ],
            "locations must sort by file, then start, then end"
        );
    }

    /// Check that the width accessors agree with the stored span.
    #[test]
    fn width_accessors_agree() {
        let location = FragmentLocation::new("src/a.rs", 4..10);

        assert_eq!(location.len(), 6, "the width must be end minus start");
        assert!(!location.is_empty(), "a non-empty span is not empty");
    }

    /// Check that an inverted span is reported empty rather than wrapping.
    #[test]
    fn inverted_span_is_empty() {
        // Bind the offsets separately: a literal reversed range is a lint
        // target, and the point here is the accessor behaviour.
        let (start, end) = (10, 4);
        let location = FragmentLocation::new("src/a.rs", start..end);

        assert_eq!(location.len(), 0, "an inverted span has no width");
        assert!(location.is_empty(), "an inverted span is empty");
    }

    /// Check that two equal locations compare equal in both directions.
    #[test]
    fn equal_locations_compare_equal() {
        let left = FragmentLocation::new("src/a.rs", 0..4);
        let right = FragmentLocation::new("src/a.rs", 0..4);

        assert_eq!(
            left.partial_cmp(&right),
            Some(Ordering::Equal),
            "equal locations must compare equal"
        );
    }
}
