//! Unit tests for the token pipeline.

use rstest::rstest;

use super::{
    Fingerprint, IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedTokenKind, ShingleSize,
    TokenPassError, WinnowWindow, hash_shingles, normalize, winnow,
};

fn labels(source: &str, profile: NormProfile) -> Vec<String> {
    normalize(source, profile)
        .map(|tokens| {
            tokens
                .into_iter()
                .map(|token| token.kind.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn t1_strips_trivia_but_keeps_identifier_and_literal_text() {
    let labels = labels("fn demo(x: i32) { /* note */ x + 1 }\n", NormProfile::T1);

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
    );

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
    assert_eq!(
        rolling
            .iter()
            .map(|fingerprint| fingerprint.range.clone())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|fingerprint| fingerprint.range.clone())
            .collect::<Vec<_>>()
    );
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
