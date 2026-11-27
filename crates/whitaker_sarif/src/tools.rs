//! Canonical SARIF producer names for the clone-detector passes.
//!
//! Run 0 and Run 1 are separate SARIF runs of the same tool. Their producer
//! names are part of the artefact contract: the future `clone_detected` lint
//! selects the AST run by name, so the names live here rather than at each call
//! site where a typo would silently change which run is read.
//!
//! # Examples
//!
//! ```
//! use whitaker_sarif::{AST_PASS_TOOL_NAME, TOKEN_PASS_TOOL_NAME};
//!
//! assert_eq!(TOKEN_PASS_TOOL_NAME, "whitaker_clones_cli@token");
//! assert_eq!(AST_PASS_TOOL_NAME, "whitaker_clones_cli@ast");
//! ```

/// Producer name recorded by the token pass (SARIF Run 0).
pub const TOKEN_PASS_TOOL_NAME: &str = "whitaker_clones_cli@token";

/// Producer name recorded by the AST pass (SARIF Run 1).
pub const AST_PASS_TOOL_NAME: &str = "whitaker_clones_cli@ast";

#[cfg(test)]
mod tests {
    //! Pin the producer names against accidental renames.

    use super::*;

    /// Check that both producer names keep their documented spelling.
    #[test]
    fn producer_names_are_stable() {
        assert_eq!(
            TOKEN_PASS_TOOL_NAME, "whitaker_clones_cli@token",
            "the token-pass producer name is part of the SARIF artefact contract"
        );
        assert_eq!(
            AST_PASS_TOOL_NAME, "whitaker_clones_cli@ast",
            "the AST-pass producer name is part of the SARIF artefact contract"
        );
    }
}
