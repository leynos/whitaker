//! Unit tests for the token pipeline.

use rstest::rstest;

use super::{
    Fingerprint, IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedTokenKind, ShingleSize,
    TokenPassError, WinnowWindow,
    fingerprint::{FNV_OFFSET_BASIS, FNV_PRIME, RABIN_KARP_BASE},
    hash_shingles, normalize, winnow,
};

fn labels(source: &str, profile: NormProfile) -> Result<Vec<String>, TokenPassError> {
    normalize(source, profile).map(|tokens| {
        tokens
            .into_iter()
            .map(|token| token.kind.to_string())
            .collect()
    })
}

fn literal_symbols(source: &str, profile: NormProfile) -> Vec<LiteralSymbol> {
    normalize(source, profile)
        .expect("literal normalization should succeed")
        .into_iter()
        .filter_map(|token| match token.kind {
            NormalizedTokenKind::Literal(symbol) => Some(symbol),
            _ => None,
        })
        .collect()
}

fn token_labels(tokens: &[super::NormalizedToken]) -> Vec<String> {
    tokens.iter().map(|token| token.kind.to_string()).collect()
}

#[test]
fn t1_strips_trivia_but_keeps_identifier_and_literal_text() {
    let labels = labels("fn demo(x: i32) { /* note */ x + 1 }\n", NormProfile::T1)
        .expect("normalization should succeed");

    assert_eq!(
        labels,
        vec![
            "fn", "demo", "(", "x", ":", "i32", ")", "{", "x", "+", "1", "}"
        ]
    );
}

#[test]
fn t2_canonicalizes_identifiers_literals_and_lifetimes() {
    let labels = labels(
        "fn demo<'a>(x: i32) { x + 1; let y = 'a; }",
        NormProfile::T2,
    )
    .expect("normalization should succeed");

    assert_eq!(
        labels,
        vec![
            "fn", "<ID_0>", "<", "<ID_0>", ">", "(", "<ID_1>", ":", "<ID_2>", ")", "{", "<ID_1>",
            "+", "<NUM>", ";", "let", "<ID_3>", "=", "<ID_0>", ";", "}"
        ]
    );
}

#[test]
fn byte_ranges_point_to_original_source() {
    let source = "fn demo() { value + 1 }";
    let tokens = normalize(source, NormProfile::T1).expect("normalization should succeed");
    let value = &tokens[5];

    assert_eq!(value.range, 12..17);
    assert_eq!(source.get(value.range.clone()), Some("value"));
}

#[test]
fn zero_sizes_are_rejected() {
    assert_eq!(
        ShingleSize::try_from(0),
        Err(TokenPassError::ZeroShingleSize)
    );
    assert_eq!(
        WinnowWindow::try_from(0),
        Err(TokenPassError::ZeroWinnowWindow)
    );
}

#[test]
fn shorter_than_k_yields_no_hashes() {
    let tokens = normalize("fn demo() {}", NormProfile::T2).expect("normalization should succeed");
    let hashes = hash_shingles(&tokens, ShingleSize::try_from(32).expect("validated size"));

    assert!(hashes.is_empty());
}

#[test]
fn exact_k_tokens_yields_one_hash() {
    let tokens = normalize("fn demo() {}", NormProfile::T2).expect("normalization should succeed");
    let hashes = hash_shingles(
        &tokens,
        ShingleSize::try_from(tokens.len()).expect("validated size"),
    );

    assert_eq!(hashes.len(), 1);
    assert_eq!(hashes[0].range, 0..12);
}

#[test]
fn rolling_hash_matches_naive_recomputation() {
    let tokens = normalize("fn demo(x: i32) { x + x + 2 }", NormProfile::T2)
        .expect("normalization should succeed");
    let width = ShingleSize::try_from(4).expect("validated size");
    let rolling = hash_shingles(&tokens, width);

    // Mirror the production constants deliberately so the test cross-checks the
    // rolling implementation against an equivalent naive recomputation.
    let expected = tokens
        .windows(width.get())
        .map(|window| {
            Fingerprint::new(
                window.iter().fold(0_u64, |hash, token| {
                    let mut value = FNV_OFFSET_BASIS;
                    value = value.wrapping_mul(FNV_PRIME)
                        ^ u64::from(match token.kind {
                            NormalizedTokenKind::Atom(_) => b'a',
                            NormalizedTokenKind::Identifier(_) => b'i',
                            NormalizedTokenKind::Lifetime(_) => b'l',
                            NormalizedTokenKind::Literal(_) => b'v',
                        });
                    for byte in token.kind.to_string().bytes() {
                        value = value.wrapping_mul(FNV_PRIME) ^ u64::from(byte);
                    }
                    hash.wrapping_mul(RABIN_KARP_BASE).wrapping_add(value)
                }),
                window
                    .first()
                    .expect("window must be non-empty")
                    .range
                    .start
                    ..window.last().expect("window must be non-empty").range.end,
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(rolling.len(), expected.len());
    for (rolling_fp, expected_fp) in rolling.iter().zip(expected.iter()) {
        assert_eq!(rolling_fp.range, expected_fp.range);
        assert_eq!(rolling_fp.hash, expected_fp.hash);
    }
}

#[test]
fn winnow_uses_rightmost_minimum() {
    let retained = winnow(
        &[
            Fingerprint::new(9, 0..1),
            Fingerprint::new(4, 1..2),
            Fingerprint::new(4, 2..3),
            Fingerprint::new(7, 3..4),
        ],
        WinnowWindow::try_from(3).expect("validated size"),
    );

    assert_eq!(retained, vec![Fingerprint::new(4, 2..3)]);
}

#[test]
fn short_winnow_input_keeps_global_minimum_once() {
    let retained = winnow(
        &[Fingerprint::new(8, 0..2), Fingerprint::new(3, 2..4)],
        WinnowWindow::try_from(5).expect("validated size"),
    );

    assert_eq!(retained, vec![Fingerprint::new(3, 2..4)]);
}

#[test]
fn normalization_is_deterministic() {
    let first =
        normalize("let value = value + 1;", NormProfile::T2).expect("normalization should succeed");
    let second =
        normalize("let value = value + 1;", NormProfile::T2).expect("normalization should succeed");

    assert_eq!(first, second);
}

#[rstest]
#[case((
    "let value = \u{00A0};",
    TokenPassError::UnsupportedToken { start: 12, end: 14 }
))]
#[case((
    "let value = /* open",
    TokenPassError::UnterminatedBlockComment { start: 12, end: 19 }
))]
#[case((
    "let value = \"open",
    TokenPassError::UnterminatedLiteral { literal_kind: "string", start: 12, end: 17 }
))]
fn invalid_source_returns_error(#[case] case: (&str, TokenPassError)) {
    let (source, expected) = case;
    let actual = normalize(source, NormProfile::T1);

    assert_eq!(actual, Err(expected));
}

#[test]
fn normalized_kinds_are_readable() {
    assert_eq!(NormalizedTokenKind::Atom("fn").to_string(), "fn");
    assert_eq!(
        NormalizedTokenKind::Identifier(IdentifierSymbol::Canonical(2)).to_string(),
        "<ID_2>"
    );
    assert_eq!(
        NormalizedTokenKind::Literal(LiteralSymbol::Canonical("<NUM>")).to_string(),
        "<NUM>"
    );
}

#[test]
fn shebang_is_stripped_equivalently_to_shebang_free_source() {
    let source_with_shebang = "#!/usr/bin/env rustc\nfn main() { let x = 1; }";
    let with_shebang = normalize(source_with_shebang, NormProfile::T1)
        .expect("normalization with shebang should succeed");

    let without_shebang =
        normalize("fn main() { let x = 1; }", NormProfile::T1).expect("shebang-free source");

    assert_eq!(
        token_labels(&with_shebang),
        token_labels(&without_shebang),
        "shebang should not affect the normalized token kinds"
    );
    assert_eq!(
        with_shebang[0].range.start,
        source_with_shebang.find("fn").expect("fn present")
    );
}

#[test]
fn raw_ident_normalizes_like_regular_ident_in_t1_and_t2() {
    let src_raw = "fn main() { r#foo(); r#foo(); }";
    let src_plain = "fn main() { foo(); foo(); }";

    let t1_raw = normalize(src_raw, NormProfile::T1).expect("T1 raw ident");
    let t1_plain = normalize(src_plain, NormProfile::T1).expect("T1 plain ident");
    assert_eq!(
        token_labels(&t1_raw),
        token_labels(&t1_plain),
        "raw identifiers should normalize to the same token stream as plain identifiers in T1"
    );

    let t2_raw = normalize(src_raw, NormProfile::T2).expect("T2 raw ident");
    let t2_plain = normalize(src_plain, NormProfile::T2).expect("T2 plain ident");
    assert_eq!(
        token_labels(&t2_raw),
        token_labels(&t2_plain),
        "raw identifiers should normalize to the same token stream as plain identifiers in T2"
    );
}

#[test]
fn raw_ident_keyword_is_treated_as_identifier_not_keyword() {
    let src = "fn main() { let r#match = 1; let x = r#match + 1; }";
    let normalized = normalize(src, NormProfile::T1).expect("raw keyword ident");

    let match_occurrences = normalized
        .iter()
        .filter(|token| {
            matches!(
                &token.kind,
                NormalizedTokenKind::Identifier(IdentifierSymbol::Original(symbol))
                    if symbol == "match"
            )
        })
        .count();

    assert_eq!(
        match_occurrences, 2,
        "expected two identifier occurrences for normalized `r#match`"
    );
}

#[rstest]
#[case(
    r#"fn main() { let _ = r"foo"; let _ = r"foo"; }"#,
    "equal raw string literals should canonicalize to the same symbol"
)]
#[case(
    r#"fn main() { let _ = br"foo"; let _ = br"foo"; }"#,
    "equal raw byte string literals should canonicalize to the same symbol"
)]
fn literal_variants_are_terminated_and_canonicalized(
    #[case] source: &str,
    #[case] assertion_message: &str,
) {
    let literal_syms = literal_symbols(source, NormProfile::T1);

    assert!(
        literal_syms.len() >= 2,
        "expected at least two literal tokens for the repeated literal pair"
    );
    assert_eq!(literal_syms[0], literal_syms[1], "{assertion_message}");
}

#[test]
fn weak_keywords_normalize_as_atoms() {
    let labels = labels(
        "fn main() { macro_rules! demo { () => {} } let _ = raw + safe + gen; }",
        NormProfile::T1,
    )
    .expect("normalization should succeed");

    assert!(labels.iter().any(|label| label == "macro_rules"));
    assert!(labels.iter().any(|label| label == "raw"));
    assert!(labels.iter().any(|label| label == "safe"));
    assert!(labels.iter().any(|label| label == "gen"));
}
