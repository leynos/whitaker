//! Attribute-specific conveniences built atop the shared path helper.

use crate::path::SimplePath;

/// Structured representation of an attribute path such as `tokio::test`.
pub type AttributePath = SimplePath;

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::AttributePath;

    #[rstest]
    fn parses_paths() {
        let path = AttributePath::from("tokio::test");
        assert_eq!(path.segments(), &["tokio", "test"]);
    }

    #[rstest]
    fn recognizes_doc_paths() {
        assert!(AttributePath::from("doc").is_doc());
        assert!(!AttributePath::from("allow").is_doc());
    }
}
