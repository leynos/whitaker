//! Unit tests for the token pipeline.

use rstest::rstest;

use super::{
    Fingerprint, IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedTokenKind, ShingleSize,
    TokenPassError, WinnowWindow, hash_shingles, normalize, winnow,
};

fn labels(source: &str, profile: NormProfile) -> Result<Vec<String>, TokenPassError> {
    normalize(source, profile).map(|tokens| {
        tokens
            .into_iter()
            .map(|token| token.kind.to_string())
            .collect()
    })
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

    let expected = tokens
        .windows(width.get())
        .map(|window| {
            Fingerprint::new(
                window.iter().fold(0_u64, |hash, token| {
                    let mut value = 0xcbf2_9ce4_8422_2325_u64;
                    for byte in token.kind.to_string().bytes() {
                        value = value.wrapping_mul(0x0000_0100_0000_01b3) ^ u64::from(byte);
                    }
                    hash.wrapping_mul(1_000_003).wrapping_add(value)
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
    let with_shebang = normalize(
        "#!/usr/bin/env rustc\nfn main() { let x = 1; }",
        NormProfile::T1,
    )
    .expect("normalization with shebang should succeed");

    let without_shebang =
        normalize("fn main() { let x = 1; }", NormProfile::T1).expect("shebang-free source");

    assert_eq!(
        with_shebang, without_shebang,
        "shebang should be fully stripped from the token stream"
    );
}

#[test]
fn raw_ident_normalizes_like_regular_ident_in_t1_and_t2() {
    let src_raw = "fn main() { r#foo(); r#foo(); }";
    let src_plain = "fn main() { foo(); foo(); }";

    let t1_raw = normalize(src_raw, NormProfile::T1).expect("T1 raw ident");
    let t1_plain = normalize(src_plain, NormProfile::T1).expect("T1 plain ident");
    assert_eq!(
        t1_raw, t1_plain,
        "raw identifiers should normalize to the same token stream as plain identifiers in T1"
    );

    let t2_raw = normalize(src_raw, NormProfile::T2).expect("T2 raw ident");
    let t2_plain = normalize(src_plain, NormProfile::T2).expect("T2 plain ident");
    assert_eq!(
        t2_raw, t2_plain,
        "raw identifiers should normalize to the same token stream as plain identifiers in T2"
    );
}

#[test]
fn raw_ident_keyword_is_treated_as_identifier_not_keyword() {
    let src = "fn main() { let r#match = 1; let x = r#match + 1; }";
    let normalized = normalize(src, NormProfile::T1).expect("raw keyword ident");

    let mut seen_id_symbol = None;
    let mut occurrences = 0_usize;

    for token in &normalized {
        if let NormalizedTokenKind::Identifier(sym) = &token.kind {
            match seen_id_symbol {
                None => {
                    seen_id_symbol = Some(sym.clone());
                    occurrences += 1;
                }
                Some(ref s) if s == sym => {
                    occurrences += 1;
                }
                _ => {}
            }
        }
    }

    assert!(
        occurrences >= 2,
        "expected at least two occurrences of the same identifier symbol for `r#match`"
    );
}

#[test]
fn raw_string_literal_is_terminated_and_canonicalized() {
    let src = r#"fn main() { let _ = r"foo"; let _ = r"foo"; }"#;
    let normalized = normalize(src, NormProfile::T1).expect("raw string normalization");

    let mut literal_syms = Vec::new();
    for token in &normalized {
        if let NormalizedTokenKind::Literal(sym) = &token.kind {
            literal_syms.push(sym.clone());
        }
    }

    assert!(
        literal_syms.len() >= 2,
        "expected at least two literal tokens for the two raw string literals"
    );
    assert_eq!(
        literal_syms[0], literal_syms[1],
        "equal raw string literals should canonicalize to the same symbol"
    );
}

#[test]
fn raw_byte_string_literal_is_terminated_and_canonicalized() {
    let src = r#"fn main() { let _ = br"foo"; let _ = br"foo"; }"#;
    let normalized = normalize(src, NormProfile::T1).expect("raw byte string normalization");

    let mut literal_syms = Vec::new();
    for token in &normalized {
        if let NormalizedTokenKind::Literal(sym) = &token.kind {
            literal_syms.push(sym.clone());
        }
    }

    assert!(
        literal_syms.len() >= 2,
        "expected at least two literal tokens for the two raw byte string literals"
    );
    assert_eq!(
        literal_syms[0], literal_syms[1],
        "equal raw byte string literals should canonicalize to the same symbol"
    );
}
