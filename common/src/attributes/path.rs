//! Attribute-specific conveniences built atop the shared path helper.

use crate::path::Path;

/// Structured representation of an attribute path such as `tokio::test`.
pub type AttributePath = Path<String>;

#[cfg(test)]
mod tests {
    use super::AttributePath;
    use rstest::rstest;

    #[rstest]
    fn parses_paths() {
        let path = AttributePath::from("tokio::test");
        assert_eq!(path.segments(), &["tokio", "test"]);
    }

    #[rstest]
    fn recognises_doc_paths() {
        assert!(AttributePath::from("doc").is_doc());
        assert!(!AttributePath::from("allow").is_doc());
    }
}
