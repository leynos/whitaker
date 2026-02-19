//! Pure heuristics for detecting example sections in documentation text.

/// The documentation pattern that triggers the lint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DocExampleViolation {
    ExamplesHeading,
    CodeFence,
}

impl DocExampleViolation {
    pub(crate) const fn note_detail(self) -> &'static str {
        match self {
            Self::ExamplesHeading => "an examples heading",
            Self::CodeFence => "a fenced code block",
        }
    }
}

/// Detect the first example-like violation in documentation text.
#[must_use]
pub(crate) fn detect_example_violation(doc_text: &str) -> Option<DocExampleViolation> {
    for line in doc_text.lines() {
        if is_examples_heading(line) {
            return Some(DocExampleViolation::ExamplesHeading);
        }
        if is_code_fence(line) {
            return Some(DocExampleViolation::CodeFence);
        }
    }

    None
}

fn is_examples_heading(line: &str) -> bool {
    let trimmed = line.trim_start();
    let heading_level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if heading_level == 0 {
        return false;
    }

    let remainder = trimmed[heading_level..].trim_start();
    matches!(
        remainder
            .trim_end_matches(|ch: char| ch.is_ascii_whitespace())
            .to_ascii_lowercase()
            .as_str(),
        "examples" | "examples:"
    )
}

fn is_code_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    let tick_count = trimmed.chars().take_while(|ch| *ch == '`').count();
    tick_count >= 3
}

#[cfg(test)]
mod tests {
    use super::{DocExampleViolation, detect_example_violation};
    use rstest::rstest;

    #[rstest]
    #[case("No examples here.", None)]
    #[case("# Examples", Some(DocExampleViolation::ExamplesHeading))]
    #[case("## Examples", Some(DocExampleViolation::ExamplesHeading))]
    #[case("###   Examples", Some(DocExampleViolation::ExamplesHeading))]
    #[case("# examples", Some(DocExampleViolation::ExamplesHeading))]
    #[case("# Examples:\nDetails", Some(DocExampleViolation::ExamplesHeading))]
    #[case("```rust\nassert!(true);\n```", Some(DocExampleViolation::CodeFence))]
    #[case("   ```\nlet a = 1;\n```", Some(DocExampleViolation::CodeFence))]
    #[case("This has inline `ticks` only.", None)]
    #[case("Heading\n# Example", None)]
    fn detects_expected_patterns(
        #[case] doc_text: &str,
        #[case] expected: Option<DocExampleViolation>,
    ) {
        assert_eq!(detect_example_violation(doc_text), expected);
    }

    #[rstest]
    fn prefers_first_match_in_source_order() {
        let doc_text = "```rust\nassert!(true);\n```\n# Examples";
        assert_eq!(
            detect_example_violation(doc_text),
            Some(DocExampleViolation::CodeFence)
        );
    }
}
