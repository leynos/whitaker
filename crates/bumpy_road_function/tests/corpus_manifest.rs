//! Consistency checks for the RFC 0013 differential corpus manifest.
//!
//! `corpus/reference.toml` records, per function, the `CodeScene` verdict and the
//! expected outcome of each strategy. These tests keep the manifest and the
//! files on disk in step and cross-check the weighted column against the
//! blessed UI `.stderr` files. They never call `CodeScene`, and they do not hash
//! the sources, so a reformat cannot break them.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use std::{collections::BTreeSet, fs, path::PathBuf};

use rstest::{fixture, rstest};
use toml::{Table, Value};

/// One function's row, with the source it belongs to.
struct Case<'a> {
    path: &'a str,
    row: &'a Table,
}

/// Return the crate directory that owns the corpus.
fn crate_dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// Report whether a file name has the `rs` extension.
fn is_rust_source(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}

/// Parse the manifest into its TOML table.
#[fixture]
fn manifest() -> Table {
    let path = crate_dir().join("corpus/reference.toml");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "corpus manifest {} should be readable: {error}",
            path.display()
        )
    });
    text.parse::<Table>()
        .unwrap_or_else(|error| panic!("corpus manifest should be valid TOML: {error}"))
}

/// Read a required string field from a table.
fn text<'a>(table: &'a Table, key: &str) -> &'a str {
    table
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("table {table:?} should have string field `{key}`"))
}

/// Return the `[[source]]` tables of the manifest.
fn sources(manifest: &Table) -> Vec<&Table> {
    manifest
        .get("source")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("manifest should contain [[source]] entries"))
        .iter()
        .map(|source| {
            source
                .as_table()
                .unwrap_or_else(|| panic!("each source should be a table"))
        })
        .collect()
}

/// Flatten every `[[source.case]]` row, keeping its source path.
fn cases(manifest: &Table) -> Vec<Case<'_>> {
    sources(manifest)
        .into_iter()
        .flat_map(|source| {
            let path = text(source, "path");
            source
                .get("case")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("source `{path}` should have at least one case"))
                .iter()
                .map(move |row| Case {
                    path,
                    row: row
                        .as_table()
                        .unwrap_or_else(|| panic!("each case should be a table")),
                })
        })
        .collect()
}

#[rstest]
fn every_corpus_source_has_an_entry_and_every_entry_a_source(manifest: Table) {
    let listed: BTreeSet<&str> = sources(&manifest)
        .into_iter()
        .map(|source| text(source, "path"))
        .collect();

    for path in &listed {
        assert!(
            crate_dir().join(path).is_file(),
            "manifest lists `{path}` but no such file exists"
        );
    }

    for directory in ["corpus", "corpus/reference_only"] {
        let entries = fs::read_dir(crate_dir().join(directory))
            .unwrap_or_else(|error| panic!("`{directory}` should be readable: {error}"));
        for entry in entries.map(|entry| entry.expect("directory entry should be readable")) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("{directory}/{name}");
            assert!(
                !is_rust_source(&name) || listed.contains(relative.as_str()),
                "`{relative}` has no entry in corpus/reference.toml"
            );
        }
    }
}

#[rstest]
fn every_case_records_a_verdict_and_an_expectation(manifest: Table) {
    let version = manifest
        .get("codescene")
        .and_then(Value::as_table)
        .map(|table| text(table, "version"))
        .expect("manifest should record the CodeScene version");
    assert!(
        !version.is_empty(),
        "the CodeScene version should not be empty"
    );

    let mut seen = BTreeSet::new();
    for case in cases(&manifest) {
        let function = text(case.row, "function");
        assert!(
            seen.insert((case.path, function)),
            "duplicate case for {function} in {}",
            case.path
        );
        assert!(
            case.row
                .get("regions")
                .and_then(Value::as_integer)
                .is_some(),
            "case {function} in {} should record an expected region count",
            case.path
        );
        assert!(
            matches!(
                text(case.row, "weighted"),
                "silent" | "flagged" | "not-compiled"
            ),
            "case {function} in {} has an unknown `weighted` outcome",
            case.path
        );
        let verdict = text(case.row, "codescene_verdict");
        assert!(
            matches!(verdict, "clean" | "bumpy"),
            "case {function} in {} has unknown verdict `{verdict}`",
            case.path
        );
        assert_eq!(
            verdict == "bumpy",
            case.row.contains_key("codescene_bumps"),
            "case {function} in {} should record `codescene_bumps` exactly when bumpy",
            case.path
        );
    }
}

#[rstest]
fn weighted_column_matches_the_blessed_stderr(manifest: Table) {
    for case in cases(&manifest) {
        let outcome = text(case.row, "weighted");
        if outcome == "not-compiled" {
            continue;
        }
        let function = text(case.row, "function");
        let stderr_path = crate_dir().join(case.path).with_extension("stderr");
        let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
        let is_flagged = stderr.contains(&format!("in `{function}`"));
        assert_eq!(
            is_flagged,
            outcome == "flagged",
            "weighted outcome for {function} in {} disagrees with {}",
            case.path,
            stderr_path.display()
        );
    }
}
