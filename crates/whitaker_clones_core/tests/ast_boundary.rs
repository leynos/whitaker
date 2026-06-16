//! Regression guard for the AST parser-adapter boundary.

use std::fs;
use std::path::Path;

const DOMAIN_FILES: &[&str] = &[
    "src/ast/error.rs",
    "src/ast/tree.rs",
    "src/ast/cover.rs",
    "src/ast/features.rs",
    "src/ast/hash.rs",
    "src/ast/mod.rs",
];
const FORBIDDEN_CRATES: &[&str] = &["ra_ap_syntax", "ra_ap_parser", "rowan"];
const FORBIDDEN_DOMAIN_IMPORT: &str = "ast::lowering";

#[test]
fn ast_domain_files_do_not_import_parser_crates() -> Result<(), Box<dyn std::error::Error>> {
    for relative_path in DOMAIN_FILES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        let contents = fs::read_to_string(&path)?;
        assert_domain_boundary(&path, &contents);
    }

    Ok(())
}

fn assert_domain_boundary(path: &Path, contents: &str) {
    for (index, line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let code = strip_line_comment(line);

        for forbidden in FORBIDDEN_CRATES {
            assert!(
                !uses_forbidden_crate(code, forbidden),
                "{}:{} must not depend on parser crate `{}`",
                path.display(),
                line_number,
                forbidden,
            );
        }

        assert!(
            !code.contains(FORBIDDEN_DOMAIN_IMPORT),
            "{}:{} must not depend on `{}`",
            path.display(),
            line_number,
            FORBIDDEN_DOMAIN_IMPORT,
        );
    }
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//")
        .map_or(line, |(before_comment, _)| before_comment)
}

fn uses_forbidden_crate(code: &str, forbidden: &str) -> bool {
    let trimmed = code.trim_start();
    let forbidden_import = format!("use {forbidden}");
    let forbidden_path = format!("{forbidden}::");

    trimmed.starts_with(&forbidden_import) || code.contains(&forbidden_path)
}
