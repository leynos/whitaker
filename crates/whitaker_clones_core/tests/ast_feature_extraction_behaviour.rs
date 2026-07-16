//! Behaviour-driven coverage for AST feature extraction.
//!
//! Keep this harness in sync with
//! `tests/features/ast_feature_extraction.feature`.

use std::cell::RefCell;

use ra_ap_syntax::SyntaxKind;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_clones_core::{
    AstError, AstHash, ByteSpan, NormalizedTree, canonical_hash, lower_span,
};

#[derive(Debug, Clone, Copy)]
enum SnippetName {
    AddFunction,
    AddExpression,
    RenamedFunctionA,
    RenamedFunctionB,
    DifferentStructure,
}

impl SnippetName {
    fn parse(s: &str) -> Self {
        match s {
            "add_function" => Self::AddFunction,
            "add_expression" => Self::AddExpression,
            "renamed_function_a" => Self::RenamedFunctionA,
            "renamed_function_b" => Self::RenamedFunctionB,
            "different_structure" => Self::DifferentStructure,
            other => panic!("unknown AST feature snippet: {other}"),
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::AddFunction => "fn add(a: i32, b: i32) -> i32 { a + b }",
            Self::AddExpression => "a + b",
            Self::RenamedFunctionA => "fn alpha(total: i32) -> i32 { total + 1 }",
            Self::RenamedFunctionB => "fn beta(count: i32) -> i32 { count + 1 }",
            Self::DifferentStructure => {
                "fn beta(count: i32) -> i32 { if count > 0 { count } else { 0 } }"
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ExpectedKind {
    BinExpr,
    SourceFile,
}

impl ExpectedKind {
    fn parse(s: &str) -> Self {
        match s {
            "BIN_EXPR" => Self::BinExpr,
            "SOURCE_FILE" => Self::SourceFile,
            other => panic!("unknown syntax kind: {other}"),
        }
    }

    fn syntax_kind(self) -> SyntaxKind {
        match self {
            Self::BinExpr => SyntaxKind::BIN_EXPR,
            Self::SourceFile => SyntaxKind::SOURCE_FILE,
        }
    }
}

#[derive(Debug, Default)]
struct AstFeatureWorld {
    source: RefCell<String>,
    span_needle: RefCell<String>,
    lowered: RefCell<Option<NormalizedTree>>,
    lowering_error: RefCell<Option<AstError>>,
    left_source: RefCell<String>,
    right_source: RefCell<String>,
    left_hash: RefCell<Option<AstHash>>,
    right_hash: RefCell<Option<AstHash>>,
}

#[fixture]
fn world() -> AstFeatureWorld {
    AstFeatureWorld::default()
}

fn whole_source_hash(source: &str) -> Result<AstHash, AstError> {
    let span = ByteSpan::new(source, 0, source.len() as u32)?;
    Ok(canonical_hash(&lower_span(source, span)?))
}

#[given("the source snippet {name}")]
fn given_source(world: &AstFeatureWorld, name: String) {
    *world.source.borrow_mut() = SnippetName::parse(&name).source().to_owned();
}

#[given("the candidate span snippet {name}")]
fn given_candidate_span(world: &AstFeatureWorld, name: String) {
    *world.span_needle.borrow_mut() = SnippetName::parse(&name).source().to_owned();
}

#[given("the left source snippet {name}")]
fn given_left_source(world: &AstFeatureWorld, name: String) {
    *world.left_source.borrow_mut() = SnippetName::parse(&name).source().to_owned();
}

#[given("the right source snippet {name}")]
fn given_right_source(world: &AstFeatureWorld, name: String) {
    *world.right_source.borrow_mut() = SnippetName::parse(&name).source().to_owned();
}

#[when("the candidate span is lowered")]
fn when_candidate_span_is_lowered(world: &AstFeatureWorld) {
    let source = world.source.borrow();
    let needle = world.span_needle.borrow();
    let Some(start) = source.find(needle.as_str()) else {
        *world.lowering_error.borrow_mut() = Some(AstError::UnparsableSpan { start: 0, end: 0 });
        return;
    };
    let end = start + needle.len();

    match ByteSpan::new(&source, start as u32, end as u32)
        .and_then(|span| lower_span(&source, span))
    {
        Ok(tree) => {
            *world.lowered.borrow_mut() = Some(tree);
            *world.lowering_error.borrow_mut() = None;
        }
        Err(error) => {
            *world.lowered.borrow_mut() = None;
            *world.lowering_error.borrow_mut() = Some(error);
        }
    }
}

#[when("both whole sources are lowered and hashed")]
fn when_both_whole_sources_are_lowered_and_hashed(world: &AstFeatureWorld) -> Result<(), AstError> {
    *world.left_hash.borrow_mut() = Some(whole_source_hash(&world.left_source.borrow())?);
    *world.right_hash.borrow_mut() = Some(whole_source_hash(&world.right_source.borrow())?);
    Ok(())
}

#[then("the lowered root kind is {kind}")]
fn then_lowered_root_kind_is(world: &AstFeatureWorld, kind: String) {
    assert_eq!(
        *world.lowering_error.borrow(),
        None,
        "lowering should not fail"
    );
    let expected = u16::from(ExpectedKind::parse(&kind).syntax_kind());
    let lowered = world.lowered.borrow();
    let tree = lowered
        .as_ref()
        .expect("lowered tree should be available after lowering");

    assert_eq!(tree.root().kind().get(), expected);
}

#[then("the AST hashes match")]
fn then_ast_hashes_match(world: &AstFeatureWorld) {
    assert_eq!(*world.left_hash.borrow(), *world.right_hash.borrow());
}

#[then("the AST hashes differ")]
fn then_ast_hashes_differ(world: &AstFeatureWorld) {
    assert_ne!(*world.left_hash.borrow(), *world.right_hash.borrow());
}

/// The `#[scenario]` macro runs every Gherkin step for this scenario before
/// this body executes; no additional assertions are required here.
#[scenario(path = "tests/features/ast_feature_extraction.feature", index = 0)]
fn scenario_smallest_covering_expression_is_selected(world: AstFeatureWorld) {
    let _ = world;
}

/// The `#[scenario]` macro runs every Gherkin step for this scenario before
/// this body executes; no additional assertions are required here.
#[scenario(path = "tests/features/ast_feature_extraction.feature", index = 1)]
fn scenario_identifier_renamed_fragments_share_an_ast_hash(world: AstFeatureWorld) {
    let _ = world;
}

/// The `#[scenario]` macro runs every Gherkin step for this scenario before
/// this body executes; no additional assertions are required here.
#[scenario(path = "tests/features/ast_feature_extraction.feature", index = 2)]
fn scenario_structurally_different_fragments_have_different_ast_hash(world: AstFeatureWorld) {
    let _ = world;
}
