//! Regression guard for the AST parser-adapter boundary.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use rustc_lexer::{LiteralKind, TokenKind, tokenize};

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

    for file in domain_files {
        assert_domain_boundary(&file.path, &file.contents);
    }

    Ok(())
}

struct DomainFile {
    path: Utf8PathBuf,
    contents: String,
}

fn domain_files() -> Result<Vec<DomainFile>, std::io::Error> {
    let ast_path = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ast");
    let ast_directory = Dir::open_ambient_dir(&ast_path, ambient_authority())?;
    let mut files = Vec::new();

    for entry in ast_directory.entries()? {
        let entry = entry?;
        let filename = entry.file_name()?;
        let path = ast_path.join(&filename);
        let should_include =
            path.extension() == Some("rs") && !ADAPTER_OR_TEST_FILES.contains(&filename.as_str());
        if should_include {
            files.push(DomainFile {
                path,
                contents: ast_directory.read_to_string(&filename)?,
            });
        }
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn assert_domain_boundary(path: &Utf8Path, contents: &str) {
    let tokens = non_comment_tokens(contents);
    assert_no_forbidden_paths(path, &tokens);

    for import in use_trees(contents) {
        for forbidden in FORBIDDEN_CRATES {
            assert!(
                !imports_crate(&import, forbidden),
                "{} must not depend on parser crate `{forbidden}` via `{}`",
                path,
                import.join(" "),
            );
        }
        // Flatten grouped/nested use trees so every sibling branch is checked,
        // not only the first one.
        for leaf in expand_use_tree(&import) {
            assert!(
                !contains_path(&leaf, FORBIDDEN_DOMAIN_IMPORT),
                "{} must not depend on `{}` via `{}`",
                path,
                FORBIDDEN_DOMAIN_IMPORT.join(""),
                import.join(" "),
            );
        }
    }
}

fn assert_no_forbidden_paths(path: &Utf8Path, tokens: &[&str]) {
    for forbidden in FORBIDDEN_CRATES {
        assert!(
            !contains_path(tokens, &["extern", "crate", forbidden]),
            "{} must not declare parser crate `{forbidden}` via `{}`",
            path,
            tokens.join(" "),
        );
        assert!(
            !contains_path(tokens, &[forbidden, "::"]),
            "{} must not reference parser crate `{forbidden}` via `{}`",
            path,
            tokens.join(" "),
        );
    }
    assert!(
        !contains_path(tokens, FORBIDDEN_DOMAIN_IMPORT),
        "{} must not reference `{}` via `{}`",
        path,
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
    let raw_tokens = non_comment_lexemes(contents);
    coalesce_path_separators(&raw_tokens)
}

fn non_comment_lexemes(contents: &str) -> Vec<&str> {
    let mut offset = 0;
    tokenize(contents)
        .filter_map(|token| {
            let end = offset + token.len;
            let lexeme = &contents[offset..end];
            offset = end;
            is_non_comment_lexeme(token.kind).then(|| normalize_raw_identifier(token.kind, lexeme))
        })
        .collect()
}

fn is_non_comment_lexeme(kind: TokenKind) -> bool {
    !matches!(
        kind,
        TokenKind::Whitespace
            | TokenKind::LineComment
            | TokenKind::BlockComment { .. }
            | TokenKind::Literal {
                kind: LiteralKind::Str { .. }
                    | LiteralKind::ByteStr { .. }
                    | LiteralKind::RawStr { .. }
                    | LiteralKind::RawByteStr { .. },
                ..
            }
    )
}

fn normalize_raw_identifier(kind: TokenKind, lexeme: &str) -> &str {
    if kind == TokenKind::RawIdent {
        lexeme.strip_prefix("r#").unwrap_or(lexeme)
    } else {
        lexeme
    }
}

fn coalesce_path_separators<'a>(tokens: &[&'a str]) -> Vec<&'a str> {
    let mut coalesced = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        if tokens[index] == ":" && tokens.get(index + 1) == Some(&":") {
            coalesced.push("::");
            index += 2;
        } else {
            coalesced.push(tokens[index]);
            index += 1;
        }
    }

    coalesced
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

/// Expands a `use`-tree token stream into every leaf path it denotes, so a
/// grouped or nested tree is checked branch by branch rather than only along
/// its first branch. For example
/// `crate :: ast :: { tree :: ByteSpan , lowering :: lower_span }` expands to
/// `[[crate, ::, ast, ::, tree, ::, ByteSpan],`
/// ` [crate, ::, ast, ::, lowering, ::, lower_span]]`, so `contains_path` sees
/// `ast :: lowering` regardless of the sibling ordering.
fn expand_use_tree<'a>(tokens: &[&'a str]) -> Vec<Vec<&'a str>> {
    parse_use_tree(tokens, &[]).0
}

/// Parses one use-tree item — a path prefix optionally followed by a `{ … }`
/// group — returning the leaf paths it expands to and the unconsumed remainder
/// (positioned at a `,`, `}`, or end of input).
fn parse_use_tree<'a>(tokens: &[&'a str], prefix: &[&'a str]) -> (Vec<Vec<&'a str>>, usize) {
    let mut path = prefix.to_vec();
    let mut index = 0;

    while index < tokens.len() {
        match tokens[index] {
            "{" => {
                let (leaves, consumed) = parse_group(&tokens[index + 1..], &path);
                return (leaves, index + 1 + consumed);
            }
            "," | "}" => break,
            "as" => index += if index + 1 < tokens.len() { 2 } else { 1 },
            segment => {
                path.push(segment);
                index += 1;
            }
        }
    }

    (vec![path], index)
}

/// Parses the comma-separated siblings of a `{ … }` group (the opening `{`
/// already consumed), combining each with `prefix`. Returns the leaf paths and
/// the number of tokens consumed, including the closing `}`.
fn parse_group<'a>(tokens: &[&'a str], prefix: &[&'a str]) -> (Vec<Vec<&'a str>>, usize) {
    let mut leaves = Vec::new();
    let mut position = 0;

    loop {
        let (sibling_leaves, consumed) = parse_use_tree(&tokens[position..], prefix);
        leaves.extend(sibling_leaves);
        position += consumed;

        match tokens.get(position) {
            Some(&",") => position += 1,
            Some(&"}") => {
                position += 1;
                break;
            }
            _ => break, // malformed or truncated input; stop defensively
        }
    }

    (leaves, position)
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

#[test]
#[should_panic(expected = "must not declare parser crate `ra_ap_syntax`")]
fn aliased_extern_crate_is_rejected() {
    let tokens = non_comment_tokens("extern crate ra_ap_syntax as syntax;");

    assert_no_forbidden_paths(Utf8Path::new("domain.rs"), &tokens);
}

#[rstest]
#[case::comment("// use ra_ap_syntax::SyntaxNode;")]
#[case::string("const EXAMPLE: &str = \"use ra_ap_syntax::SyntaxNode;\";")]
fn non_import_text_does_not_trigger_the_boundary_guard(#[case] source: &str) {
    assert!(use_trees(source).is_empty());
}

#[test]
fn non_comment_tokens_discard_comments_and_strings_but_keep_paths() {
    let tokens = non_comment_tokens(
        "// hidden_comment\nconst HIDDEN: &str = \"hidden string\";\nuse crate::ast::tree::ByteSpan;",
    );

    assert!(!tokens.iter().any(|token| token.contains("hidden_comment")));
    assert!(!tokens.iter().any(|token| token.contains("hidden string")));
    assert!(contains_path(
        &tokens,
        &["crate", "::", "ast", "::", "tree", "::", "ByteSpan"],
    ));
    assert!(tokens.contains(&"use"));
}

#[rstest]
#[case::direct("use crate::ast::lowering::lower_span;")]
#[case::re_exported("pub use crate::ast::lowering::lower_span;")]
#[case::grouped("use crate::ast::{lowering::lower_span};")]
#[case::grouped_later_sibling("use crate::ast::{tree::ByteSpan, lowering::lower_span};")]
#[case::nested_group_later_sibling("use crate::ast::{tree::{ByteSpan}, lowering::lower_span};")]
fn forbidden_domain_import_forms_are_detected(#[case] source: &str) {
    let import = use_trees(source).pop().expect("source contains a use tree");

    assert!(
        expand_use_tree(&import)
            .iter()
            .any(|leaf| contains_path(leaf, FORBIDDEN_DOMAIN_IMPORT))
    );
}

#[rstest]
#[case::parser_crate("type Syntax = ra_ap_syntax::SyntaxNode;")]
#[case::domain_module("const LOWER: &str = crate::ast::lowering::PARSER_SCHEMA_VERSION;")]
#[case::raw_parser_crate("type Syntax = r#ra_ap_syntax::SyntaxNode;")]
#[case::raw_domain_module("const LOWER: &str = crate::r#ast::lowering::PARSER_SCHEMA_VERSION;")]
fn inline_forbidden_paths_are_detected(#[case] source: &str) {
    let tokens = non_comment_tokens(source);

    assert!(
        FORBIDDEN_CRATES
            .iter()
            .any(|crate_name| contains_path(&tokens, &[crate_name, "::"]))
            || contains_path(&tokens, FORBIDDEN_DOMAIN_IMPORT)
    );
}
