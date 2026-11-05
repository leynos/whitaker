//! Property tests for source reconstruction and evaluated-value preservation.

use proptest::prelude::*;

use crate::{
    continuation::{Continuation, joins_only},
    facts::LiteralBody,
    props::body_with_continuations,
    rewrite::{
        ConcatRewrite,
        gate::{decoded_value, preserves_value},
    },
};

/// The fragment alphabet this suite generates.
///
/// Wider than the range suite's by the emoji and its `\u{...}` spelling,
/// because these bodies are decoded and compared, so the escapes that only
/// differ after decoding need to appear.
const ALPHABET: &[&str] = &["alpha", "λ", "🦄", "\\\"", "\\\\", "\\u{1F984}", "{{", "}}"];

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

fn assert_preserved(body: &str, expected_joins: usize) -> Result<(), TestCaseError> {
    let wrapped = LiteralBody::new(body);
    let joins = joins_only(wrapped.scan());
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

    let Some(original_value) = decoded_value(body) else {
        return Err(TestCaseError::fail("generated original body must unescape"));
    };
    let Some(fragment_values) = rewrite
        .fragments()
        .iter()
        .map(|fragment| decoded_value(fragment))
        .collect::<Option<Vec<_>>>()
    else {
        return Err(TestCaseError::fail("generated fragments must unescape"));
    };
    prop_assert_eq!(fragment_values.concat(), original_value.clone());
    prop_assert!(preserves_value(body, rewrite.fragments()));
    Ok(())
}

proptest! {
    #[test]
    fn rewrites_preserve_source_spelling_and_evaluated_value(
        (body, expected_joins) in body_with_continuations(ALPHABET)
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
            crate::props::append_continuation(&mut body, (1, 0, 0));
        }
        if has_non_bmp && fragment == 0 {
            body.push('🦄');
        }
        body.push_str("alpha");
    }
    body
}
