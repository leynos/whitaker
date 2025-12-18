//! Markdown TOML block extraction utilities.
//!
//! This module provides helpers for extracting TOML code blocks from markdown
//! documentation files. Extracted blocks are used to validate that documented
//! examples parse correctly as TOML.

use std::sync::LazyLock;

/// Path to the user guide relative to the workspace root.
pub const USERS_GUIDE_PATH: &str = "docs/users-guide.md";

/// Extracted TOML code blocks from the user guide, loaded once at test startup.
pub static DOC_TOML_BLOCKS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .expect("installer crate should be in workspace");
    let guide_path = workspace_root.join(USERS_GUIDE_PATH);

    let content = std::fs::read_to_string(&guide_path).expect("failed to read users-guide.md");

    extract_toml_blocks(&content)
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
/// ```
/// # use behaviour_docs::extraction::extract_toml_blocks;
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
            "expected TOML blocks from users-guide.md"
        );
    }
}
