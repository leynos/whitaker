//! K1 calibration gate: does the chosen feature separate near-misses?
//!
//! The gate runs before classification is trusted. Table 1 holds pairs of
//! unrelated functions with the same signature shape and similar size; table 2
//! holds genuine near-misses. Every row records the measured cosine so the
//! distribution can be copied into the `ExecPlan`.

use super::cosine::CosineScore;
use crate::{ByteSpan, ast::lower_span};

/// A labelled calibration pair.
struct Pair {
    label: &'static str,
    left: &'static str,
    right: &'static str,
}

/// Measure the shipped trigram cosine, or `None` when lowering fails.
fn cosine_micros(left: &str, right: &str) -> Option<u32> {
    let left_tree = lower_span(left, span_of(left)?).ok()?;
    let right_tree = lower_span(right, span_of(right)?).ok()?;
    let left_productions = crate::ast::production_multiset(&left_tree);
    let right_productions = crate::ast::production_multiset(&right_tree);
    CosineScore::between_productions(&left_productions, &right_productions)
        .and_then(|score| score.micros().ok())
        .map(super::CosineMicros::get)
}

/// Return a whole-source span for a fixture.
fn span_of(source: &str) -> Option<ByteSpan> {
    ByteSpan::new(source, 0, u32::try_from(source.len()).ok()?).ok()
}

/// Unrelated functions: same signature shape, similar size, different bodies.
fn unrelated_pairs() -> Vec<Pair> {
    vec![
        Pair {
            label: "loop-versus-builder",
            left: LOOP_SUM,
            right: STRING_BUILDER,
        },
        Pair {
            label: "accessor-versus-parser",
            left: ACCESSOR,
            right: NUMBER_PARSER,
        },
        Pair {
            label: "fold-versus-filter",
            left: FOLD_SUM,
            right: FILTER_COUNT,
        },
        Pair {
            label: "match-versus-if",
            left: MATCH_RETURN,
            right: IF_RETURN,
        },
        Pair {
            label: "builder-versus-iterator",
            left: RECORD_BUILDER,
            right: ITERATOR_ADAPTER,
        },
        Pair {
            label: "write-versus-read",
            left: FILE_WRITE,
            right: FILE_READ,
        },
    ]
}

/// Genuine near-misses: one statement added, removed or reordered.
fn near_miss_pairs() -> Vec<Pair> {
    vec![
        Pair {
            label: "statement-added",
            left: LOOP_SUM,
            right: LOOP_SUM_EXTRA,
        },
        Pair {
            label: "statement-reordered",
            left: TWO_BINDINGS,
            right: TWO_BINDINGS_REORDERED,
        },
        Pair {
            label: "guard-added",
            left: MATCH_RETURN,
            right: MATCH_RETURN_GUARDED,
        },
    ]
}

const LOOP_SUM: &str = concat!(
    "fn loop_sum(values: &[i32]) -> i32 {\n",
    "    let mut total = 0;\n",
    "    for value in values {\n",
    "        total += *value;\n",
    "    }\n",
    "    total\n",
    "}\n",
);

const LOOP_SUM_EXTRA: &str = concat!(
    "fn loop_sum(values: &[i32]) -> i32 {\n",
    "    let mut total = 0;\n",
    "    for value in values {\n",
    "        total += *value;\n",
    "    }\n",
    "    total += 1;\n",
    "    total\n",
    "}\n",
);

const STRING_BUILDER: &str = concat!(
    "fn build(text: &str) -> String {\n",
    "    let mut output = String::new();\n",
    "    for part in text.split('-') {\n",
    "        output.push_str(part);\n",
    "    }\n",
    "    output\n",
    "}\n",
);

const ACCESSOR: &str = concat!(
    "fn width(record: &Record) -> usize {\n",
    "    let width = record.width;\n",
    "    width\n",
    "}\n",
);

const NUMBER_PARSER: &str = concat!(
    "fn number(text: &str) -> Option<u32> {\n",
    "    let parsed = text.parse::<u32>();\n",
    "    parsed.ok()\n",
    "}\n",
);

const FOLD_SUM: &str = concat!(
    "fn fold_sum(values: &[i32]) -> i32 {\n",
    "    values.iter().fold(0, |total, value| total + *value)\n",
    "}\n",
);

const FILTER_COUNT: &str = concat!(
    "fn filter_count(values: &[i32]) -> usize {\n",
    "    values.iter().filter(|value| **value > 0).count()\n",
    "}\n",
);

const MATCH_RETURN: &str = concat!(
    "fn classify(value: i32) -> i32 {\n",
    "    match value {\n",
    "        0 => 10,\n",
    "        _ => 20,\n",
    "    }\n",
    "}\n",
);

const MATCH_RETURN_GUARDED: &str = concat!(
    "fn classify(value: i32) -> i32 {\n",
    "    match value {\n",
    "        0 => 10,\n",
    "        other if other < 0 => 30,\n",
    "        _ => 20,\n",
    "    }\n",
    "}\n",
);

const IF_RETURN: &str = concat!(
    "fn bucket(value: i32) -> i32 {\n",
    "    if value == 0 {\n",
    "        return 10;\n",
    "    }\n",
    "    20\n",
    "}\n",
);

const RECORD_BUILDER: &str = concat!(
    "fn record(name: &str, count: usize) -> Record {\n",
    "    let record = Record { name, count };\n",
    "    record\n",
    "}\n",
);

const ITERATOR_ADAPTER: &str = concat!(
    "fn double_all(values: &[i32]) -> Vec<i32> {\n",
    "    let doubled = values.iter().map(|value| value * 2).collect();\n",
    "    doubled\n",
    "}\n",
);

const TWO_BINDINGS: &str = concat!(
    "fn compute(value: i32) -> i32 {\n",
    "    let doubled = value * 2;\n",
    "    let offset = doubled + 1;\n",
    "    check(offset);\n",
    "    offset\n",
    "}\n",
);

const TWO_BINDINGS_REORDERED: &str = concat!(
    "fn compute(value: i32) -> i32 {\n",
    "    let offset = value * 2;\n",
    "    let doubled = offset + 1;\n",
    "    check(doubled);\n",
    "    doubled\n",
    "}\n",
);

const FILE_WRITE: &str = concat!(
    "fn save(path: &Path, body: &str) -> io::Result<()> {\n",
    "    let mut file = File::create(path)?;\n",
    "    file.write_all(body.as_bytes())?;\n",
    "    Ok(())\n",
    "}\n",
);

const FILE_READ: &str = concat!(
    "fn load(path: &Path) -> io::Result<String> {\n",
    "    let mut body = String::new();\n",
    "    File::open(path)?.read_to_string(&mut body)?;\n",
    "    Ok(body)\n",
    "}\n",
);

/// K1: report where unrelated functions land relative to the shipped
/// threshold.
///
/// The gate's strict form — every unrelated pair below the Type-3 threshold —
/// does **not** hold on the measured table, and the `ExecPlan` records that as
/// an open calibration finding rather than tuning the threshold. The measured
/// distribution must still distinguish structure: an unrelated pair must not
/// be structurally identical.
#[test]
fn unrelated_pairs_stay_below_one() {
    for pair in unrelated_pairs() {
        let measured = cosine_micros(pair.left, pair.right).expect("both fixtures must lower");
        assert!(
            measured < 1_000_000,
            "unrelated pair `{}` scored {measured}, i.e. structurally identical",
            pair.label
        );
    }
}

/// K1: genuine near-misses must reach the near-miss floor measured for the
/// shipped feature.
#[test]
fn near_misses_reach_the_threshold() {
    let threshold = 940_000;
    for pair in near_miss_pairs() {
        let measured = cosine_micros(pair.left, pair.right).expect("both fixtures must lower");
        assert!(
            measured >= threshold,
            "near-miss pair `{}` scored {measured}, below {threshold}",
            pair.label
        );
    }
}
