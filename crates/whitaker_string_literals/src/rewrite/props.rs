//! Property tests for source reconstruction and evaluated-value preservation.

use proptest::prelude::*;
use rustc_lexer::unescape::unescape_str;

use crate::{
    continuation::{Continuation, ContinuationKind},
    facts::LiteralBody,
    rewrite::ConcatRewrite,
};

fn source_piece() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("alpha"),
            Just("λ"),
            Just("🦄"),
            Just("\\\""),
            Just("\\\\"),
            Just("\\u{1F984}"),
            Just("{{"),
            Just("}}"),
        ],
        1..4,
    )
    .prop_map(|pieces| pieces.concat())
}

fn append_continuation(body: &mut String, whitespace: (u8, u8, u8)) {
    let (spaces, blank_lines, tabs) = whitespace;
    body.push('\\');
    body.push('\n');
    for _ in 0..spaces {
        body.push(' ');
    }
    for _ in 0..blank_lines {
        body.push('\n');
    }
    for _ in 0..tabs {
        body.push('\t');
    }
}

fn body_with_joins() -> impl Strategy<Value = (String, usize)> {
    prop::collection::vec(source_piece(), 2..=6).prop_flat_map(|source_fragments| {
        let join_count = source_fragments.len().saturating_sub(1);
        (
            Just(source_fragments),
            prop::collection::vec((1_u8..4, 0_u8..3, 0_u8..3), join_count),
        )
            .prop_map(|(body_fragments, whitespace)| {
                let mut body = String::new();
                for (index, fragment) in body_fragments.iter().enumerate() {
                    body.push_str(fragment);
                    whitespace
                        .get(index)
                        .copied()
                        .into_iter()
                        .for_each(|settings| append_continuation(&mut body, settings));
                }
                (body, body_fragments.len().saturating_sub(1))
            })
    })
}

fn joins_only(body: LiteralBody<'_>) -> Vec<Continuation> {
    body.scan()
        .into_iter()
        .filter(|continuation| continuation.kind == ContinuationKind::Join)
        .collect()
}

fn remove_ranges(body: &str, joins: &[Continuation]) -> Option<String> {
    let mut output = String::new();
    let mut previous_end = 0;
    for join in joins {
        let start = join.escape_range.start.get();
        let end = join.skipped_whitespace_range.end.get();
        output.push_str(body.get(previous_end..start)?);
        previous_end = end;
    }
    output.push_str(body.get(previous_end..)?);
    Some(output)
}

fn unescaped_value(body: &str) -> Option<String> {
    let mut value = String::new();
    let mut has_error = false;
    unescape_str(body, &mut |_range, result| match result {
        Ok(character) => value.push(character),
        Err(_) => has_error = true,
    });
    (!has_error).then_some(value)
}

fn assert_preserved(body: &str, expected_joins: usize) -> Result<(), TestCaseError> {
    let wrapped = LiteralBody::new(body);
    let joins = joins_only(wrapped);
    prop_assert_eq!(joins.len(), expected_joins);

    let Some(rewrite) = ConcatRewrite::build(wrapped, &joins) else {
        return Err(TestCaseError::fail("a generated body must be rewritable"));
    };

    let joined_fragments = rewrite.fragments().concat();
    let source_without_joins = remove_ranges(body, &joins);
    prop_assert_eq!(
        source_without_joins.as_deref(),
        Some(joined_fragments.as_str())
    );

    let Some(original_value) = unescaped_value(body) else {
        return Err(TestCaseError::fail("generated original body must unescape"));
    };
    let Some(fragment_values) = rewrite
        .fragments()
        .iter()
        .map(|fragment| unescaped_value(fragment))
        .collect::<Option<Vec<_>>>()
    else {
        return Err(TestCaseError::fail("generated fragments must unescape"));
    };
    prop_assert_eq!(fragment_values.concat(), original_value);
    Ok(())
}

proptest! {
    #[test]
    fn rewrites_preserve_source_spelling_and_evaluated_value(
        (body, expected_joins) in body_with_joins()
    ) {
        assert_preserved(&body, expected_joins)?;
    }
}

#[test]
fn deterministic_matrix_covers_each_join_count_and_non_bmp_presence() {
    let mut class_counts = [[0_usize; 2]; 5];

    for join_count in 1..=5 {
        for has_non_bmp in [false, true] {
            let body = deterministic_body(join_count, has_non_bmp);

            assert_preserved(&body, join_count)
                .expect("the deterministic matrix must preserve source and value");
            let join_class = join_count.saturating_sub(1);
            let non_bmp_class = usize::from(has_non_bmp);
            let counts = class_counts
                .get_mut(join_class)
                .expect("join count is within the matrix bound");
            let count = counts
                .get_mut(non_bmp_class)
                .expect("non-BMP class is within the matrix bound");
            *count = count.saturating_add(1);
        }
    }

    for counts in class_counts {
        assert!(
            counts.first().is_some_and(|count| *count > 0),
            "each join count needs a BMP-only sample"
        );
        assert!(
            counts.get(1).is_some_and(|count| *count > 0),
            "each join count needs a non-BMP sample"
        );
    }
}

fn deterministic_body(join_count: usize, has_non_bmp: bool) -> String {
    let mut body = String::new();
    for fragment in 0..=join_count {
        if fragment > 0 {
            append_continuation(&mut body, (1, 0, 0));
        }
        if has_non_bmp && fragment == 0 {
            body.push('🦄');
        }
        body.push_str("alpha");
    }
    body
}
