//! Dylint driver bootstrap for the `rstest` helper fixture lint.
//!
//! The driver owns compiler integration and configuration loading. Pure
//! configuration normalization stays in small helper methods so it can be
//! tested without constructing rustc contexts.

use crate::collector::CallSiteCollector;
use crate::visitor::{
    CallSiteVisitor, attribute_from_hir, fixture_local_ids, redacted_fingerprint_shape,
};
use log::debug;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use serde::Deserialize;
use std::collections::HashSet;
use std::io::Write;
use whitaker::SharedConfig;
use whitaker_common::attributes::AttributePath;
use whitaker_common::i18n::{Localizer, get_localizer_for_lint};
use whitaker_common::rstest::{RstestDetectionOptions, is_rstest_test_with};

const LINT_NAME: &str = "rstest_helper_should_be_fixture";
/// Internal test-only hook used by the UI harness to assert passive collection.
///
/// The lint remains diagnostic-silent for users. When this variable is set by
/// tests, crate-post writes an append-only summary so the harness can verify
/// that the pass-owned collector observed real rustc call-site evidence. The
/// summary includes callee and record counts, plus redacted fingerprint shape
/// tokens such as `fixture-local`, `const-lit`, `const-path`, and `unsupported`.
const COLLECTION_SUMMARY_ENV: &str = "WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY";

const DEFAULT_PROVIDER_PARAM_ATTRIBUTES: &[&str] =
    &["case", "values", "files", "future", "context"];

type ConfigLoadResult = Result<Config, String>;

dylint_linting::impl_late_lint! {
    pub RSTEST_HELPER_SHOULD_BE_FIXTURE,
    Warn,
    "repeated rstest helper calls should be extracted into fixtures",
    RstestHelperShouldBeFixture::default()
}

/// Configuration for the `rstest_helper_should_be_fixture` lint.
///
/// Values are loaded from `dylint.toml` and normalized so threshold settings
/// keep repeated-helper semantics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Config {
    min_calls: usize,
    min_distinct_tests: usize,
    require_identical_fixture_arg_names: bool,
    provider_param_attributes: Vec<String>,
    use_source_callee_fallback: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_calls: 2,
            min_distinct_tests: 2,
            require_identical_fixture_arg_names: false,
            provider_param_attributes: DEFAULT_PROVIDER_PARAM_ATTRIBUTES
                .iter()
                .map(ToString::to_string)
                .collect(),
            use_source_callee_fallback: false,
        }
    }
}

impl Config {
    fn normalized(self) -> Self {
        Self {
            min_calls: self.min_calls.max(2),
            min_distinct_tests: self.min_distinct_tests.max(2),
            require_identical_fixture_arg_names: self.require_identical_fixture_arg_names,
            provider_param_attributes: normalize_provider_attributes(
                self.provider_param_attributes,
            ),
            use_source_callee_fallback: self.use_source_callee_fallback,
        }
    }

    fn detection_options(&self) -> RstestDetectionOptions {
        let provider_param_attributes = self
            .provider_param_attributes
            .iter()
            .flat_map(|attribute| {
                [
                    AttributePath::from(attribute.as_str()),
                    AttributePath::from(format!("rstest::{attribute}")),
                ]
            })
            .collect();
        RstestDetectionOptions::new(provider_param_attributes, self.use_source_callee_fallback)
    }
}

/// Lint pass bootstrap for repeated `rstest` helper extraction.
pub struct RstestHelperShouldBeFixture {
    config: Config,
    detection_options: RstestDetectionOptions,
    collector: CallSiteCollector,
    rstest_collection_roots: HashSet<hir::HirId>,
    localizer: Localizer,
}

impl Default for RstestHelperShouldBeFixture {
    fn default() -> Self {
        let config = Config::default();
        let detection_options = config.detection_options();
        Self {
            config,
            detection_options,
            collector: CallSiteCollector::default(),
            rstest_collection_roots: HashSet::new(),
            localizer: Localizer::new(None),
        }
    }
}

impl RstestHelperShouldBeFixture {
    fn apply_loaded_crate_configuration(
        &mut self,
        config: ConfigLoadResult,
        shared_config: SharedConfig,
    ) {
        let config = match config {
            Ok(config) => config,
            Err(error) => {
                debug!(
                    target: LINT_NAME,
                    "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
                );
                Config::default()
            }
        };

        self.apply_crate_configuration(config, shared_config);
    }

    fn apply_crate_configuration(&mut self, config: Config, shared_config: SharedConfig) {
        debug!(
            target: LINT_NAME,
            "applying `{LINT_NAME}` configuration: min_calls={}, min_distinct_tests={}, \
             require_identical_fixture_arg_names={}, provider_param_attributes={:?}, \
             use_source_callee_fallback={}, locale={:?}",
            config.min_calls,
            config.min_distinct_tests,
            config.require_identical_fixture_arg_names,
            config.provider_param_attributes,
            config.use_source_callee_fallback,
            shared_config.locale(),
        );
        self.config = config;
        self.detection_options = self.config.detection_options();
        self.collector.clear();
        self.rstest_collection_roots.clear();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn collect_call_sites<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        def_id: LocalDefId,
    ) {
        let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
        let attrs = cx
            .tcx
            .hir_attrs(hir_id)
            .iter()
            .filter_map(attribute_from_hir)
            .collect::<Vec<_>>();
        if !is_rstest_test_with(&attrs, None, &self.detection_options)
            && !self.rstest_collection_roots.contains(&hir_id)
        {
            debug!(
                target: LINT_NAME,
                "skipping helper call-site collection for non-rstest function: def_id={:?}",
                def_id,
            );
            return;
        }

        let fixture_local_ids = fixture_local_ids(cx, body, &self.detection_options);
        let mut visitor = CallSiteVisitor::new(
            cx,
            &mut self.collector,
            def_id.to_def_id(),
            &fixture_local_ids,
        );
        visitor.visit_expr(body.value);
    }
}

impl<'tcx> LateLintPass<'tcx> for RstestHelperShouldBeFixture {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.apply_loaded_crate_configuration(load_configuration(), load_shared_config());
        self.rstest_collection_roots = whitaker::hir::collect_rstest_companion_test_functions(cx);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _kind: hir::intravisit::FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        self.collect_call_sites(cx, body, def_id);
    }

    fn check_crate_post(&mut self, _cx: &LateContext<'tcx>) {
        for (callee, records) in self.collector.iter() {
            for record in records {
                debug!(
                    target: LINT_NAME,
                    "collected rstest helper call: callee={}, callee_def_id={:?}, \
                     test_source_def_id={:?}, span={:?}, fingerprint_shape={}",
                    callee,
                    record.callee_def_id,
                    record.test_source_def_id,
                    record.span,
                    redacted_fingerprint_shape(&record.fingerprint),
                );
            }
        }
        if let Err(error) = self.write_collection_summary() {
            debug!(
                target: LINT_NAME,
                "failed to write rstest helper call-site collection summary: {}",
                error,
            );
        }
        debug!(
            target: LINT_NAME,
            "rstest helper call-site collection complete: {} callees, {} records",
            self.collector.callee_count(),
            self.collector.record_count(),
        );
    }
}

impl RstestHelperShouldBeFixture {
    fn collection_summary(&self) -> String {
        let mut summary = format!(
            "callee_count={}\nrecord_count={}\n",
            self.collector.callee_count(),
            self.collector.record_count(),
        );
        for (callee, records) in self.collector.iter() {
            summary.push_str(&format!("callee={callee};records={}\n", records.len()));
            for record in records {
                summary.push_str(&format!(
                    "fingerprint={}\n",
                    redacted_fingerprint_shape(&record.fingerprint)
                ));
            }
        }
        summary
    }

    fn write_collection_summary(&self) -> std::io::Result<()> {
        let Some(path) = std::env::var_os(COLLECTION_SUMMARY_ENV) else {
            return Ok(());
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{}", self.collection_summary())
    }
}

fn normalize_provider_attributes(attributes: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for attribute in attributes
        .into_iter()
        .map(|attribute| attribute.trim().trim_start_matches("rstest::").to_owned())
        .filter(|attribute| !attribute.is_empty())
    {
        if !normalized.contains(&attribute) {
            normalized.push(attribute);
        }
    }

    if normalized.is_empty() {
        return default_provider_param_attributes();
    }

    normalized
}

fn default_provider_param_attributes() -> Vec<String> {
    DEFAULT_PROVIDER_PARAM_ATTRIBUTES
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn load_configuration() -> ConfigLoadResult {
    debug!(target: LINT_NAME, "loading `{LINT_NAME}` configuration");
    loaded_configuration(dylint_linting::config::<Config>(LINT_NAME))
}

fn load_shared_config() -> SharedConfig {
    // SAFETY / NOTE: `SharedConfig::load` does not currently propagate I/O
    // errors, so this named boundary documents the infallible call site
    // pending https://github.com/leynos/whitaker/issues/233.
    debug!(target: LINT_NAME, "loading shared Whitaker configuration");
    SharedConfig::load()
}

fn loaded_configuration<E>(loaded: Result<Option<Config>, E>) -> ConfigLoadResult
where
    E: std::fmt::Display,
{
    match loaded {
        Ok(Some(config)) => {
            debug!(target: LINT_NAME, "loaded explicit `{LINT_NAME}` configuration");
            Ok(config.normalized())
        }
        Ok(None) => {
            debug!(target: LINT_NAME, "no `{LINT_NAME}` configuration found; using defaults");
            Ok(Config::default())
        }
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
