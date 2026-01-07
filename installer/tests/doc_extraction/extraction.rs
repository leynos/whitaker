//! Markdown TOML block extraction utilities.
//!
//! This module provides helpers for extracting TOML code blocks from markdown
//! documentation files. Extracted blocks are used to validate that documented
//! examples parse correctly as TOML.

use std::sync::LazyLock;

/// Paths to documentation files relative to the workspace root.
const DOC_PATHS: &[&str] = &["docs/users-guide.md", "docs/developers-guide.md"];

/// Extracted TOML code blocks from documentation, loaded once at test startup.
pub static DOC_TOML_BLOCKS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .expect("installer crate should be in workspace");

    let mut all_blocks = Vec::new();
    for path in DOC_PATHS {
        let guide_path = workspace_root.join(path);
        let content = std::fs::read_to_string(&guide_path)
            .unwrap_or_else(|_| panic!("failed to read {path}"));
        all_blocks.extend(extract_toml_blocks(&content));
    }
    all_blocks
});

/// Process a single line within a TOML code block, accumulating non-comment lines.
fn accumulate_toml_line(line: &str, current_block: &mut String) {
    // Skip comment lines for cleaner TOML
    if line.trim_start().starts_with('#') {
        return;
    }
    current_block.push_str(line);
    current_block.push('\n');
}

/// Extract all TOML code blocks from markdown content.
///
/// Skips comment lines (starting with `#`) for cleaner TOML extraction.
/// This produces structural TOML that can be validated for correctness.
///
/// # Examples
///
/// ```ignore
/// use doc_extraction::extraction::extract_toml_blocks;
///
/// let markdown = r#"
/// ```toml
/// # A comment
/// key = "value"
/// ```
/// "#;
/// let blocks = extract_toml_blocks(markdown);
/// assert_eq!(blocks.len(), 1);
/// assert!(!blocks[0].contains("# A comment"));
/// ```
pub fn extract_toml_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_toml_block = false;
    let mut current_block = String::new();

    for line in markdown.lines() {
        if line.starts_with("```toml") {
            in_toml_block = true;
            current_block.clear();
            continue;
        }

        if !in_toml_block {
            continue;
        }

        if line.starts_with("```") {
            in_toml_block = false;
            blocks.push(current_block.clone());
            continue;
        }

        accumulate_toml_line(line, &mut current_block);
    }

    blocks
}

/// Find a TOML block containing the specified marker text.
pub fn find_block_containing(marker: &str) -> String {
    DOC_TOML_BLOCKS
        .iter()
        .find(|block| block.contains(marker))
        .unwrap_or_else(|| panic!("no TOML block containing '{marker}' found in documentation"))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_toml_blocks_from_markdown() {
        let markdown = r#"
# Example

```toml
[section]
key = "value"
```

Some text.

```toml
other = true
```
"#;

        let blocks = extract_toml_blocks(markdown);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("key = \"value\""));
        assert!(blocks[1].contains("other = true"));
    }

    #[test]
    fn skips_comment_lines_in_toml_blocks() {
        let markdown = r#"
```toml
# This is a comment
[section]
key = "value"
```
"#;

        let blocks = extract_toml_blocks(markdown);
        assert_eq!(blocks.len(), 1);
        assert!(
            !blocks[0].contains("# This is a comment"),
            "expected comment to be skipped"
        );
        assert!(blocks[0].contains("key = \"value\""));
    }

    #[test]
    fn doc_toml_blocks_are_loaded() {
        // Verify the lazy static loaded successfully
        assert!(
            !DOC_TOML_BLOCKS.is_empty(),
            "expected TOML blocks from documentation"
        );
    }
}
