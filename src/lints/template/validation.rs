//! Validates inputs for lint crate scaffolding.

use camino::{Utf8Component, Utf8Path};

use super::TemplateError;

fn is_valid_crate_name_character(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit() || matches!(character, '-' | '_')
}

fn is_absolute_path(normalized: &str, original: &str) -> Result<(), TemplateError> {
    if normalized.starts_with("//") {
        return Err(TemplateError::AbsoluteUiDirectory {
            directory: original.to_string(),
        });
    }

    let bytes = normalized.as_bytes();
    if matches!(bytes, [drive, b':', ..] if drive.is_ascii_alphabetic()) {
        return Err(TemplateError::AbsoluteUiDirectory {
            directory: original.to_string(),
        });
    }

    let path = Utf8Path::new(normalized);
    if path.is_absolute() {
        return Err(TemplateError::AbsoluteUiDirectory {
            directory: original.to_string(),
        });
    }

    Ok(())
}

fn process_path_component<'a>(
    component: Utf8Component<'a>,
    segments: &mut Vec<&'a str>,
    original: &str,
) -> Result<(), TemplateError> {
    match component {
        Utf8Component::CurDir => Ok(()),
        Utf8Component::ParentDir => Err(TemplateError::ParentUiDirectory {
            directory: original.to_string(),
        }),
        Utf8Component::Normal(segment) => {
            if !segment.is_empty() {
                segments.push(segment);
            }
            Ok(())
        }
        Utf8Component::RootDir | Utf8Component::Prefix(_) => {
            Err(TemplateError::AbsoluteUiDirectory {
                directory: original.to_string(),
            })
        }
    }
}

pub(crate) fn normalize_crate_name(input: &str) -> Result<String, TemplateError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(TemplateError::EmptyCrateName);
    }

    let mut characters = trimmed.chars();
    let first = characters
        .next()
        .expect("crate name is non-empty after trim");

    if !first.is_ascii_lowercase() {
        return Err(TemplateError::InvalidCrateNameStart { character: first });
    }

    for character in characters {
        if !is_valid_crate_name_character(character) {
            return Err(TemplateError::InvalidCrateNameCharacter { character });
        }
    }

    // Safe: non-empty and ASCII-only by earlier validation checks.
    let last = trimmed.as_bytes()[trimmed.len() - 1] as char;
    if matches!(last, '-' | '_') {
        return Err(TemplateError::CrateNameTrailingSeparator { character: last });
    }

    Ok(trimmed.to_string())
}

pub(crate) fn lint_constant(crate_name: &str) -> String {
    crate_name
        .chars()
        .map(|character| match character {
            '-' | '_' => '_',
            other => other,
        })
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

pub(crate) fn pass_struct_name(crate_name: &str) -> String {
    crate_name
        .split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut characters = segment.chars();
            let first = characters
                .next()
                .expect("non-empty segments remain after filtering");
            let mut capitalised = String::with_capacity(segment.len());
            capitalised.push(first.to_ascii_uppercase());
            for character in characters {
                capitalised.push(character.to_ascii_lowercase());
            }
            capitalised
        })
        .collect()
}

pub(crate) fn normalize_ui_directory(input: &str) -> Result<String, TemplateError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(TemplateError::EmptyUiDirectory);
    }

    let normalized = trimmed.replace('\\', "/");
    is_absolute_path(&normalized, trimmed)?;

    let path = Utf8Path::new(&normalized);
    let mut segments = Vec::new();

    for component in path.components() {
        process_path_component(component, &mut segments, trimmed)?;
    }

    if segments.is_empty() {
        return Err(TemplateError::EmptyUiDirectory);
    }

    Ok(segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    #[rstest]
    #[case("module_max_400_lines", "MODULE_MAX_400_LINES")]
    #[case("no-expect-outside-tests", "NO_EXPECT_OUTSIDE_TESTS")]
    fn constant_from_crate_name(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(lint_constant(input), expected);
    }

    #[rstest]
    #[case("module_max_400_lines", "ModuleMax400Lines")]
    #[case("no-expect-outside-tests", "NoExpectOutsideTests")]
    fn pass_struct_from_crate_name(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(pass_struct_name(input), expected);
    }

    #[test]
    fn normalizes_nested_ui_directory() {
        let directory = normalize_ui_directory("ui/lints/expr")
            .unwrap_or_else(|error| panic!("valid path expected: {error}"));
        assert_eq!(directory, "ui/lints/expr");
    }

    #[test]
    fn normalizes_windows_separators() {
        let directory = normalize_ui_directory(r"ui\nested\cases")
            .unwrap_or_else(|error| panic!("valid path expected: {error}"));
        assert_eq!(directory, "ui/nested/cases");
    }

    #[test]
    fn normalizes_consecutive_separators() {
        let directory = normalize_ui_directory("ui//nested///cases")
            .unwrap_or_else(|error| panic!("valid path expected: {error}"));
        assert_eq!(directory, "ui/nested/cases");
    }

    #[test]
    fn normalizes_mixed_separators() {
        let directory = normalize_ui_directory(r"ui\nested//cases\more")
            .unwrap_or_else(|error| panic!("valid path expected: {error}"));
        assert_eq!(directory, "ui/nested/cases/more");
    }

    #[rstest]
    #[case(
        "ui/../secrets",
        TemplateError::ParentUiDirectory {
            directory: "ui/../secrets".to_string(),
        },
        "parent directory traversal should be rejected"
    )]
    #[case(
        "//server/share/ui",
        TemplateError::AbsoluteUiDirectory {
            directory: String::from("//server/share/ui"),
        },
        "UNC paths should be rejected"
    )]
    #[case(
        r"C:\\ui",
        TemplateError::AbsoluteUiDirectory {
            directory: String::from(r"C:\\ui"),
        },
        "absolute windows paths should be rejected"
    )]
    #[case(
        "C:ui",
        TemplateError::AbsoluteUiDirectory {
            directory: String::from("C:ui"),
        },
        "drive-letter prefixes must be rejected"
    )]
    fn rejects_invalid_ui_directory(
        #[case] input: &str,
        #[case] expected_error: TemplateError,
        #[case] panic_message: &str,
    ) {
        let Err(error) = normalize_ui_directory(input) else {
            panic!("{panic_message}");
        };
        assert_eq!(error, expected_error);
    }
}
