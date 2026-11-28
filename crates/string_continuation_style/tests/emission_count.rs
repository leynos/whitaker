//! Generated emission-count coverage for the early lint pass.
//!
//! The classifier property proves one `PreferConcat` disposition per literal,
//! but it never runs the driver. These tests generate fixture crates with
//! literals carrying one to several joins, run the real lint through
//! `cargo dylint`, and associate each emitted diagnostic with its literal's
//! primary line. A deterministic one-to-ten matrix covers the boundary; a
//! proptest drives varied safe fragment text through the same assertion. A
//! suppression on one literal and a double emission on another still fails
//! even when the aggregate count is unchanged.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
};

#[path = "support/emission.rs"]
mod support;

use anyhow::Context as _;
use cap_std::{ambient_authority, fs::Dir};
use cargo_metadata::Message;
use proptest::prelude::*;
use serial_test::serial;
use support::{LINT_CRATE_NAME, lint_library_path, pinned_toolchain};
use tempfile::{Builder, TempDir};

const MAX_JOINS: usize = 10;

/// Safe fragment spellings the generated literals draw from.
const ALPHABET: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "λ", "🦄", "{{", "}}", "\\\"", "\\\\",
];

/// Returns the workspace target directory used for generated fixtures.
///
/// Building inside the workspace target keeps the fixture's Dylint target
/// directory off `/tmp`, which is scratch space rather than a build target.
fn fixture_root() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"),
            PathBuf::from,
        )
        .join("emission-count-fixtures")
}

/// A generated fixture and the 1-based line of each literal's `const`.
struct GeneratedFixture {
    source: String,
    literal_lines: BTreeSet<usize>,
}

/// Returns a safe fragment spelling for `seed`.
fn generated_fragment(seed: u64) -> &'static str {
    let modulus = u64::try_from(ALPHABET.len()).unwrap_or(1);
    let index = usize::try_from(seed.wrapping_rem(modulus)).unwrap_or(0);
    ALPHABET.get(index).copied().unwrap_or("alpha")
}

/// Writes a fixture crate whose literals have one through ten joins.
fn create_fixture(generated: &GeneratedFixture) -> anyhow::Result<TempDir> {
    let root = fixture_root();
    Dir::create_ambient_dir_all(&root, ambient_authority())
        .context("failed to create the emission fixture root")?;
    let fixture = Builder::new()
        .prefix("emission-count-")
        .tempdir_in(&root)
        .context("failed to create the emission fixture directory")?;
    let directory = Dir::open_ambient_dir(fixture.path(), ambient_authority())
        .context("failed to open the emission fixture directory")?;
    directory
        .write(
            "Cargo.toml",
            concat!(
                "[package]\n",
                "name = \"emission-count-fixture\"\n",
                "version = \"0.1.0\"\n",
                "edition = \"2024\"\n",
                "\n",
                "[workspace]\n",
            ),
        )
        .context("failed to write the fixture manifest")?;
    directory
        .create_dir("src")
        .context("failed to create the fixture source directory")?;
    directory
        .write("src/lib.rs", &generated.source)
        .context("failed to write the fixture source")?;
    Ok(fixture)
}

/// Builds a fixture from one fragment vector per literal.
fn fixture_from_fragments(literals: &[Vec<&str>]) -> GeneratedFixture {
    let mut lines = vec![
        String::from("//! Generated emission fixture."),
        String::from("#![allow(dead_code)]"),
        String::new(),
    ];
    let mut literal_lines = BTreeSet::new();

    for (index, fragments) in literals.iter().enumerate() {
        // The next line is the `const` line; record it before appending the
        // multi-line literal body.
        literal_lines.insert(lines.len().saturating_add(1));

        let body = fragments.join(" \\\n    ");
        let mut body_lines = body.lines();
        let first = body_lines.next().unwrap_or("alpha");
        lines.push(format!("const L{index}: &str = \"{first}"));
        for line in body_lines {
            lines.push(line.to_owned());
        }
        if let Some(last) = lines.last_mut() {
            last.push_str("\";");
        }
    }

    GeneratedFixture {
        source: lines.join("\n") + "\n",
        literal_lines,
    }
}

/// Builds the deterministic one-to-ten join matrix.
fn generated_source() -> GeneratedFixture {
    let literals = (1..=MAX_JOINS)
        .map(|joins| {
            (0..=joins)
                .map(|index| {
                    let seed =
                        u64::try_from(joins.saturating_mul(31).saturating_add(index)).unwrap_or(0);
                    generated_fragment(seed)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    fixture_from_fragments(&literals)
}

/// The result of running `cargo dylint` against the fixture.
struct CargoDylintResult {
    is_success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

/// Runs `cargo dylint` for the named lint against `fixture`.
fn run_cargo_dylint(fixture: &Path, library_path: &Path) -> anyhow::Result<CargoDylintResult> {
    let output = Command::new("cargo")
        .arg("dylint")
        .arg("--lib")
        .arg(LINT_CRATE_NAME)
        .arg("--")
        .arg("--message-format")
        .arg("json")
        .current_dir(fixture)
        .env("DYLINT_LIBRARY_PATH", library_path)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain())
        .env("CARGO_NET_OFFLINE", "true")
        .env_remove("RUSTFLAGS")
        .env_remove("DYLINT_RUSTFLAGS")
        .output()
        .context("failed to execute cargo dylint")?;

    Ok(CargoDylintResult {
        is_success: output.status.success(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Counts `string_continuation_style` diagnostics by primary source line.
fn diagnostic_lines(output: &[u8]) -> anyhow::Result<BTreeMap<usize, usize>> {
    let messages = Message::parse_stream(Cursor::new(output))
        .collect::<Result<Vec<_>, _>>()
        .context("failed to parse the cargo JSON output")?;
    let mut counts = BTreeMap::new();

    for diagnostic in messages
        .into_iter()
        .filter_map(|message| match message {
            Message::CompilerMessage(compiler_message) => Some(compiler_message.message),
            _ => None,
        })
        .filter(|diagnostic| {
            diagnostic
                .code
                .as_ref()
                .is_some_and(|code| code.code == LINT_CRATE_NAME)
        })
    {
        let Some(primary) = diagnostic.spans.iter().find(|span| span.is_primary) else {
            anyhow::bail!("diagnostic has no primary span: {}", diagnostic.message);
        };
        let count = counts.entry(primary.line_start).or_insert(0_usize);
        *count = count.saturating_add(1);
    }

    Ok(counts)
}

/// Asserts exactly one diagnostic for every expected literal line.
fn assert_one_per_literal(
    counts: &BTreeMap<usize, usize>,
    expected: &BTreeSet<usize>,
) -> anyhow::Result<()> {
    for line in expected {
        let count = counts.get(line).copied().unwrap_or(0);
        anyhow::ensure!(
            count == 1,
            "literal at line {line} should have exactly one diagnostic, got {count}"
        );
    }
    for line in counts.keys() {
        anyhow::ensure!(
            expected.contains(line),
            "diagnostic at line {line} does not correspond to a generated literal"
        );
    }

    let total = counts.values().copied().sum::<usize>();
    anyhow::ensure!(
        total == expected.len(),
        "aggregate diagnostic count {total} should equal the {} generated literals",
        expected.len()
    );

    Ok(())
}

#[test]
#[serial]
fn generated_literals_emit_exactly_one_diagnostic_each() -> anyhow::Result<()> {
    let library_path = lint_library_path()?;
    let generated = generated_source();
    let fixture = create_fixture(&generated)?;
    let result = run_cargo_dylint(fixture.path(), library_path)?;

    anyhow::ensure!(
        result.is_success,
        "cargo dylint failed for the generated fixture: {}",
        result.stderr
    );
    let counts = diagnostic_lines(&result.stdout)?;
    assert_one_per_literal(&counts, &generated.literal_lines)?;

    Ok(())
}

#[test]
fn per_literal_check_rejects_a_suppression_and_a_double_with_the_same_total() {
    let expected = BTreeSet::from([10_usize, 20]);
    let counts = BTreeMap::from([(10_usize, 2_usize)]);

    let result = assert_one_per_literal(&counts, &expected);

    assert!(
        result.is_err(),
        "a double at line 10 and a suppression at line 20 must be rejected even though the total \
         is two"
    );
}

#[test]
fn per_literal_check_accepts_one_diagnostic_per_literal() {
    let expected = BTreeSet::from([10_usize, 20]);
    let counts = BTreeMap::from([(10_usize, 1_usize), (20, 1)]);

    let result = assert_one_per_literal(&counts, &expected);

    assert!(
        result.is_ok(),
        "one diagnostic on each expected line must be accepted"
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2, ..ProptestConfig::default() })]

    #[test]
    #[serial]
    fn generated_fragment_text_emits_one_diagnostic_per_literal(
        literals in prop::collection::vec(
            prop::collection::vec(prop::sample::select(ALPHABET), 2..=9),
            1..=3,
        )
    ) {
        let library_path =
            lint_library_path().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let generated = fixture_from_fragments(&literals);
        let fixture =
            create_fixture(&generated).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let result = run_cargo_dylint(fixture.path(), library_path)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert!(
            result.is_success,
            "cargo dylint failed for generated fragments: {}",
            result.stderr
        );
        let counts = diagnostic_lines(&result.stdout)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let checked = assert_one_per_literal(&counts, &generated.literal_lines);
        prop_assert!(checked.is_ok(), "{:?}", checked.err());
    }
}

#[test]
#[serial]
fn minimal_alpha_join_emits_one_diagnostic() -> anyhow::Result<()> {
    let library_path = lint_library_path()?;
    let generated = fixture_from_fragments(&[vec!["alpha", "alpha"]]);
    let fixture = create_fixture(&generated)?;
    let result = run_cargo_dylint(fixture.path(), library_path)?;

    anyhow::ensure!(
        result.is_success,
        "cargo dylint failed for the minimal fixture: {}",
        result.stderr
    );
    let counts = diagnostic_lines(&result.stdout)?;
    assert_one_per_literal(&counts, &generated.literal_lines)?;

    Ok(())
}
