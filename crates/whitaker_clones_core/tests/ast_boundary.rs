//! Regression guard for the AST parser-adapter boundary.

use std::fs;
use std::path::{Path, PathBuf};

use rstest::rstest;
use rustc_lexer::tokenize;

const FORBIDDEN_CRATES: &[&str] = &["ra_ap_syntax", "ra_ap_parser", "rowan"];
const FORBIDDEN_DOMAIN_IMPORT: &[&str] = &["ast", "::", "lowering"];
const ADAPTER_OR_TEST_FILES: &[&str] = &["kani.rs", "lowering.rs", "lowering_tests.rs", "tests.rs"];

#[test]
fn ast_domain_files_do_not_import_parser_crates() -> Result<(), Box<dyn std::error::Error>> {
    let domain_files = domain_files()?;
    assert!(
        !domain_files.is_empty(),
        "AST domain-file discovery must not be empty"
    );

    for path in domain_files {
        assert_domain_boundary(&path, &fs::read_to_string(&path)?);
    }

    Ok(())
}

fn domain_files() -> Result<Vec<PathBuf>, std::io::Error> {
    let ast_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ast");
    let mut files = fs::read_dir(ast_directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !ADAPTER_OR_TEST_FILES.contains(&name))
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn assert_domain_boundary(path: &Path, contents: &str) {
    let tokens = non_comment_tokens(contents);
    assert_no_forbidden_paths(path, &tokens);

    for import in use_trees(contents) {
        for forbidden in FORBIDDEN_CRATES {
            assert!(
                !imports_crate(&import, forbidden),
                "{} must not depend on parser crate `{forbidden}` via `{}`",
                path.display(),
                import.join(" "),
            );
        }
        assert!(
            !contains_path(&import, FORBIDDEN_DOMAIN_IMPORT),
            "{} must not depend on `{}` via `{}`",
            path.display(),
            FORBIDDEN_DOMAIN_IMPORT.join(""),
            import.join(" "),
        );
    }
}

fn assert_no_forbidden_paths(path: &Path, tokens: &[&str]) {
    for forbidden in FORBIDDEN_CRATES {
        assert!(
            !contains_path(tokens, &[forbidden, "::"]),
            "{} must not reference parser crate `{forbidden}` via `{}`",
            path.display(),
            tokens.join(" "),
        );
    }
    assert!(
        !contains_path(tokens, FORBIDDEN_DOMAIN_IMPORT),
        "{} must not reference `{}` via `{}`",
        path.display(),
        FORBIDDEN_DOMAIN_IMPORT.join(""),
        tokens.join(" "),
    );
}

fn use_trees(contents: &str) -> Vec<Vec<&str>> {
    let tokens = non_comment_tokens(contents);
    let mut imports = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        if tokens[index] != "use" {
            index += 1;
            continue;
        }

        let end = tokens[index + 1..]
            .iter()
            .position(|token| *token == ";")
            .map_or(tokens.len(), |offset| index + 1 + offset);
        imports.push(tokens[index + 1..end].to_vec());
        index = end + 1;
    }

    imports
}

fn non_comment_tokens(contents: &str) -> Vec<&str> {
    let mut offset = 0;
    let raw_tokens = tokenize(contents)
        .filter_map(|token| {
            let end = offset + token.len;
            let lexeme = &contents[offset..end];
            offset = end;
            (!lexeme.trim().is_empty()
                && !lexeme.starts_with("//")
                && !lexeme.starts_with("/*")
                && !lexeme.starts_with('"')
                && !lexeme.starts_with("r\"")
                && !lexeme.starts_with("r#"))
            .then_some(lexeme)
        })
        .collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < raw_tokens.len() {
        if raw_tokens[index] == ":" && raw_tokens.get(index + 1) == Some(&":") {
            tokens.push("::");
            index += 2;
        } else {
            tokens.push(raw_tokens[index]);
            index += 1;
        }
    }

    tokens
}

fn imports_crate(import: &[&str], forbidden: &str) -> bool {
    import.iter().enumerate().any(|(index, token)| {
        *token == forbidden
            && matches!(
                index
                    .checked_sub(1)
                    .and_then(|previous| import.get(previous)),
                None | Some(&"::") | Some(&"{") | Some(&",")
            )
    })
}

fn contains_path(import: &[&str], path: &[&str]) -> bool {
    import.windows(path.len()).any(|window| window == path)
}

#[rstest]
#[case::direct("use ra_ap_syntax::SyntaxNode;")]
#[case::qualified("use ::ra_ap_syntax::SyntaxNode;")]
#[case::aliased("use ra_ap_syntax as syntax;")]
#[case::re_exported("pub use rowan::SyntaxNode;")]
#[case::grouped("use crate::{error::AstError, ra_ap_parser::Edition};")]
fn forbidden_crate_import_forms_are_detected(#[case] source: &str) {
    let import = use_trees(source).pop().expect("source contains a use tree");

    assert!(
        FORBIDDEN_CRATES
            .iter()
            .any(|crate_name| imports_crate(&import, crate_name))
    );
}

#[rstest]
#[case::comment("// use ra_ap_syntax::SyntaxNode;")]
#[case::string("const EXAMPLE: &str = \"use ra_ap_syntax::SyntaxNode;\";")]
fn non_import_text_does_not_trigger_the_boundary_guard(#[case] source: &str) {
    assert!(use_trees(source).is_empty());
}

#[rstest]
#[case::direct("use crate::ast::lowering::lower_span;")]
#[case::re_exported("pub use crate::ast::lowering::lower_span;")]
fn forbidden_domain_import_forms_are_detected(#[case] source: &str) {
    let import = use_trees(source).pop().expect("source contains a use tree");

    assert!(contains_path(&import, FORBIDDEN_DOMAIN_IMPORT));
}

#[rstest]
#[case::parser_crate("type Syntax = ra_ap_syntax::SyntaxNode;")]
#[case::domain_module("const LOWER: &str = crate::ast::lowering::PARSER_SCHEMA_VERSION;")]
fn inline_forbidden_paths_are_detected(#[case] source: &str) {
    let tokens = non_comment_tokens(source);

    assert!(
        FORBIDDEN_CRATES
            .iter()
            .any(|crate_name| contains_path(&tokens, &[crate_name, "::"]))
            || contains_path(&tokens, FORBIDDEN_DOMAIN_IMPORT)
    );
}
