//! SARIF run merging and result deduplication.
//!
//! The clone detector produces separate SARIF runs for the token pass and AST
//! pass. This module provides [`merge_runs`] to combine them into a single run
//! and [`deduplicate_results`] to remove results with identical
//! `(fingerprint, file, region)` keys.

use std::collections::HashSet;

use crate::error::{Result, SarifError};
use crate::model::location::Region;
use crate::model::result::SarifResult;
use crate::model::run::Run;

/// Composite key used for result deduplication.
///
/// Two results are considered duplicates when they share the same Whitaker
/// fragment fingerprint, file URI, and source region.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ResultKey {
    fingerprint: String,
    file: String,
    region: RegionKey,
}

/// Hashable representation of a [`Region`] for deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RegionKey {
    start_line: usize,
    start_column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
}

impl RegionKey {
    fn from_region(region: &Region) -> Self {
        Self {
            start_line: region.start_line,
            start_column: region.start_column,
            end_line: region.end_line,
            end_column: region.end_column,
        }
    }
}

/// Attempts to extract a deduplication key from a result.
///
/// Returns `None` if the result lacks the required fingerprint, location, or
/// region data needed to form a key.
fn extract_key(result: &SarifResult) -> Option<ResultKey> {
    let fingerprint = result.partial_fingerprints.get("whitakerFragment")?.clone();

    let location = result.locations.first()?;
    let file = location.physical_location.artifact_location.uri.clone();
    let region = location.physical_location.region.as_ref()?;

    Some(ResultKey {
        fingerprint,
        file,
        region: RegionKey::from_region(region),
    })
}

/// Removes duplicate results based on `(fingerprint, file, region)`.
///
/// Results without a valid deduplication key (missing fingerprint, location,
/// or region) are always preserved.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{SarifResult, Level, Message, deduplicate_results};
///
/// let results = vec![
///     SarifResult {
///         rule_id: "WHK001".into(),
///         level: Level::Warning,
///         message: Message { text: "clone".into() },
///         locations: Vec::new(),
///         related_locations: Vec::new(),
///         partial_fingerprints: Default::default(),
///         properties: None,
///         baseline_state: None,
///     },
/// ];
/// let deduped = deduplicate_results(&results);
/// assert_eq!(deduped.len(), 1);
/// ```
#[must_use]
pub fn deduplicate_results(results: &[SarifResult]) -> Vec<SarifResult> {
    let mut seen = HashSet::new();
    let mut deduplicated = Vec::new();

    for result in results {
        match extract_key(result) {
            Some(key) => {
                if seen.insert(key) {
                    deduplicated.push(result.clone());
                }
            }
            None => {
                // Preserve results without proper deduplication keys.
                deduplicated.push(result.clone());
            }
        }
    }

    deduplicated
}

/// Merges multiple runs into a single run.
///
/// The tool metadata is taken from the first run. Results are collected from
/// all runs and deduplicated. Artifacts and invocations are concatenated.
///
/// # Errors
///
/// Returns [`SarifError::MergeConflict`] if `runs` is empty.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{RunBuilder, merge_runs};
///
/// let run_a = RunBuilder::new("tool", "1.0").build();
/// let run_b = RunBuilder::new("tool", "1.0").build();
/// let merged = merge_runs(&[run_a, run_b]).expect("merge");
/// assert_eq!(merged.tool.driver.name, "tool");
/// ```
pub fn merge_runs(runs: &[Run]) -> Result<Run> {
    let first = runs
        .first()
        .ok_or_else(|| SarifError::MergeConflict("cannot merge zero runs".into()))?;

    let tool = first.tool.clone();

    let mut all_results = Vec::new();
    let mut all_artifacts = Vec::new();
    let mut all_invocations = Vec::new();

    for run in runs {
        all_results.extend(run.results.clone());
        all_artifacts.extend(run.artifacts.clone());
        all_invocations.extend(run.invocations.clone());
    }

    let results = deduplicate_results(&all_results);

    Ok(Run {
        tool,
        invocations: all_invocations,
        results,
        artifacts: all_artifacts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{LocationBuilder, RegionBuilder, ResultBuilder, RunBuilder};
    use crate::model::result::Level;

    fn make_keyed_result(rule: &str, file: &str, line: usize, fp: &str) -> SarifResult {
        ResultBuilder::new(rule)
            .with_message("clone detected")
            .with_level(Level::Warning)
            .with_location(
                LocationBuilder::new(file)
                    .with_region(RegionBuilder::new(line).with_end_line(line + 5).build())
                    .build(),
            )
            .with_fingerprint("whitakerFragment", fp)
            .build()
            .expect("build result")
    }

    #[test]
    fn dedup_removes_exact_duplicates() {
        let r1 = make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
        let r2 = r1.clone();
        let deduped = deduplicate_results(&[r1, r2]);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn dedup_preserves_different_fingerprints() {
        let r1 = make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
        let r2 = make_keyed_result("WHK001", "src/a.rs", 10, "fp2");
        let deduped = deduplicate_results(&[r1, r2]);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn dedup_preserves_different_files() {
        let r1 = make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
        let r2 = make_keyed_result("WHK001", "src/b.rs", 10, "fp1");
        let deduped = deduplicate_results(&[r1, r2]);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn dedup_preserves_different_regions() {
        let r1 = make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
        let r2 = make_keyed_result("WHK001", "src/a.rs", 20, "fp1");
        let deduped = deduplicate_results(&[r1, r2]);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn dedup_preserves_results_without_keys() {
        let r1 = ResultBuilder::new("WHK001")
            .with_message("no location")
            .build()
            .expect("build");
        let r2 = r1.clone();
        let deduped = deduplicate_results(&[r1, r2]);
        // Both preserved because they lack deduplication keys.
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn merge_runs_combines_results() {
        let r1 = make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
        let r2 = make_keyed_result("WHK002", "src/b.rs", 20, "fp2");

        let run_a = RunBuilder::new("tool", "1.0").with_result(r1).build();
        let run_b = RunBuilder::new("tool", "1.0").with_result(r2).build();

        let merged = merge_runs(&[run_a, run_b]).expect("merge");
        assert_eq!(merged.results.len(), 2);
    }

    #[test]
    fn merge_runs_deduplicates() {
        let r1 = make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
        let r2 = r1.clone();

        let run_a = RunBuilder::new("tool", "1.0").with_result(r1).build();
        let run_b = RunBuilder::new("tool", "1.0").with_result(r2).build();

        let merged = merge_runs(&[run_a, run_b]).expect("merge");
        assert_eq!(merged.results.len(), 1);
    }

    #[test]
    fn merge_empty_runs_returns_error() {
        let result = merge_runs(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn merge_takes_tool_from_first_run() {
        let run_a = RunBuilder::new("alpha", "1.0").build();
        let run_b = RunBuilder::new("beta", "2.0").build();

        let merged = merge_runs(&[run_a, run_b]).expect("merge");
        assert_eq!(merged.tool.driver.name, "alpha");
    }
}
