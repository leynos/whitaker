use camino::{Utf8Component, Utf8Path};

use super::TemplateError;

fn is_valid_crate_name_character(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit() || matches!(character, '-' | '_')
}

pub(crate) fn normalise_crate_name(input: &str) -> Result<String, TemplateError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(TemplateError::EmptyCrateName);
    }

    let mut characters = trimmed.chars();
    let Some(first) = characters.next() else {
        return Err(TemplateError::EmptyCrateName);
    };

    if !first.is_ascii_lowercase() {
        return Err(TemplateError::InvalidCrateNameStart { character: first });
    }

    for character in characters {
        if !is_valid_crate_name_character(character) {
            return Err(TemplateError::InvalidCrateNameCharacter { character });
        }
    }

    let Some(last) = trimmed.chars().last() else {
        return Err(TemplateError::EmptyCrateName);
    };
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
            let Some(first) = characters.next() else {
                return String::new();
            };

            let mut capitalised = String::new();
            capitalised.push(first.to_ascii_uppercase());
            for character in characters {
                capitalised.push(character.to_ascii_lowercase());
            }
            capitalised
        })
        .collect()
}

pub(crate) fn normalise_ui_directory(input: &str) -> Result<String, TemplateError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(TemplateError::EmptyUiDirectory);
    }

    let normalised = trimmed.replace('\\', "/");

    if normalised.starts_with("//") {
        return Err(TemplateError::AbsoluteUiDirectory {
            directory: trimmed.to_string(),
        });
    }

    if normalised.split_once(':').is_some_and(|(prefix, rest)| {
        prefix.len() == 1
            && prefix
                .chars()
                .all(|character| character.is_ascii_alphabetic())
            && (rest.is_empty() || rest.starts_with('/'))
    }) {
        return Err(TemplateError::AbsoluteUiDirectory {
            directory: trimmed.to_string(),
        });
    }

    let path = Utf8Path::new(&normalised);
    if path.is_absolute() {
        return Err(TemplateError::AbsoluteUiDirectory {
            directory: trimmed.to_string(),
        });
    }

    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                return Err(TemplateError::ParentUiDirectory {
                    directory: trimmed.to_string(),
                });
            }
            Utf8Component::Normal(segment) => {
                if !segment.is_empty() {
                    segments.push(segment);
                }
            }
            Utf8Component::RootDir | Utf8Component::Prefix(_) => {
                return Err(TemplateError::AbsoluteUiDirectory {
                    directory: trimmed.to_string(),
                });
            }
        }
    }

    if segments.is_empty() {
        return Err(TemplateError::EmptyUiDirectory);
    }

    Ok(segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_from_crate_name() {
        assert_eq!(
            lint_constant("module_max_400_lines"),
            "MODULE_MAX_400_LINES"
        );
        assert_eq!(
            lint_constant("no-expect-outside-tests"),
            "NO_EXPECT_OUTSIDE_TESTS"
        );
    }

    #[test]
    fn pass_struct_from_crate_name() {
        assert_eq!(
            pass_struct_name("module_max_400_lines"),
            "ModuleMax400Lines"
        );
        assert_eq!(
            pass_struct_name("no-expect-outside-tests"),
            "NoExpectOutsideTests"
        );
    }

    #[test]
    fn normalises_nested_ui_directory() {
        let directory = normalise_ui_directory("ui/lints/expr")
            .unwrap_or_else(|error| panic!("valid path expected: {error}"));
        assert_eq!(directory, "ui/lints/expr");
    }

    #[test]
    fn normalises_windows_separators() {
        let directory = normalise_ui_directory(r"ui\nested\cases")
            .unwrap_or_else(|error| panic!("valid path expected: {error}"));
        assert_eq!(directory, "ui/nested/cases");
    }

    #[test]
    fn rejects_parent_directory_in_ui_path() {
        let Err(error) = normalise_ui_directory("ui/../secrets") else {
            panic!("parent directory traversal should be rejected");
        };
        assert_eq!(
            error,
            TemplateError::ParentUiDirectory {
                directory: "ui/../secrets".to_string(),
            }
        );
    }

    #[test]
    fn rejects_absolute_windows_path() {
        let Err(error) = normalise_ui_directory(r"C:\\ui") else {
            panic!("absolute windows paths should be rejected");
        };
        assert_eq!(
            error,
            TemplateError::AbsoluteUiDirectory {
                directory: String::from(r"C:\\ui"),
            }
        );
    }
}
