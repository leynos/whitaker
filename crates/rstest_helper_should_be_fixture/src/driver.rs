//! Dylint driver bootstrap for the `rstest` helper fixture lint.
//!
//! The driver owns compiler integration and configuration loading. Pure
//! configuration normalization stays in small helper methods so it can be
//! tested without constructing rustc contexts.

use crate::collector::{
    CallSiteCollector, CallSiteLocation, CallSiteRecord, lower_arg_atom, resolve_local_callee,
};
use log::debug;
use rustc_ast::AttrStyle;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use serde::Deserialize;
use whitaker::SharedConfig;
use whitaker_common::attributes::{Attribute, AttributeKind, AttributePath};
use whitaker_common::i18n::{Localizer, get_localizer_for_lint};
use whitaker_common::rstest::{
    ArgFingerprint, ParameterBinding, RstestDetectionOptions, RstestParameter, fixture_local_names,
    is_rstest_test_with,
};

const LINT_NAME: &str = "rstest_helper_should_be_fixture";

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
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn collect_rstest_fn<'tcx>(
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
        if !is_rstest_test_with(&attrs, None, &self.detection_options) {
            return;
        }

        let parameters = rstest_parameters(cx, body);
        let fixture_locals = fixture_local_names(&parameters, &self.detection_options);
        let mut visitor =
            CallSiteVisitor::new(cx, &mut self.collector, def_id.to_def_id(), &fixture_locals);
        visitor.visit_expr(body.value);
    }
}

impl<'tcx> LateLintPass<'tcx> for RstestHelperShouldBeFixture {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.apply_loaded_crate_configuration(load_configuration(), load_shared_config());
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
        self.collect_rstest_fn(cx, body, def_id);
    }

    fn check_crate_post(&mut self, _cx: &LateContext<'tcx>) {
        for (callee, records) in self.collector.iter() {

                debug!(
                    target: LINT_NAME,
                    "collected rstest helper call: callee={}, callee_def_id={:?}, \
                     test_source_def_id={:?}, span={:?}, fingerprint={:?}",
                    callee,
                    record.callee_def_id,
                    record.test_source_def_id,
                    record.span,
                    record.fingerprint,
                );
            }
        }
        debug!(
            target: LINT_NAME,
            "rstest helper call-site collection complete: {} callees, {} records",
            self.collector.callee_count(),
            self.collector.record_count(),
        );
    }
}

struct CallSiteVisitor<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    collector: &'a mut CallSiteCollector,
    test_source_def_id: DefId,
    fixture_locals: &'a std::collections::BTreeSet<String>,
}

impl<'a, 'tcx> CallSiteVisitor<'a, 'tcx> {
    fn new(
        cx: &'a LateContext<'tcx>,
        collector: &'a mut CallSiteCollector,
        test_source_def_id: DefId,
        fixture_locals: &'a std::collections::BTreeSet<String>,
    ) -> Self {
        Self {
            cx,
            collector,
            test_source_def_id,
            fixture_locals,
        }
    }

    fn collect_call(&mut self, expr: &'tcx hir::Expr<'tcx>, args: &'tcx [hir::Expr<'tcx>]) {
        let Some(span) = whitaker::hir::recover_user_editable_hir_span(expr.span) else {
            return;
        };
        let Some(callee_def_id) = resolve_local_callee(self.cx, expr) else {
            return;
        };

        let fingerprint = ArgFingerprint::new(
            args.iter()
                .map(|arg| lower_arg_atom(self.cx, arg, self.fixture_locals)),
        );
        let record = CallSiteRecord::new(callee_def_id, fingerprint, self.test_source_def_id, span);
        let source_map = self.cx.tcx.sess.source_map();
        self.collector.record(
            record,
            CallSiteLocation::new(
                self.cx.tcx.def_path_str(callee_def_id),
                source_map.span_to_filename(span),
                span.lo(),
                span.hi(),
            ),
        );
    }
}

impl<'tcx> Visitor<'tcx> for CallSiteVisitor<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        match expr.kind {
            hir::ExprKind::Call(_, args) => self.collect_call(expr, args),
            hir::ExprKind::MethodCall(_, _, args, _) => self.collect_call(expr, args),
            hir::ExprKind::Closure(..) => return,
            _ => {}
        }

        intravisit::walk_expr(self, expr);
    }
}

fn rstest_parameters(cx: &LateContext<'_>, body: &hir::Body<'_>) -> Vec<RstestParameter> {
    body.params
        .iter()
        .map(|param| {
            RstestParameter::new(
                parameter_binding(param.pat),
                parameter_attributes(cx, param.pat.hir_id),
            )
        })
        .collect()
}

fn parameter_binding(pat: &hir::Pat<'_>) -> ParameterBinding {
    match pat.kind {
        hir::PatKind::Binding(_, _, ident, None) => ParameterBinding::Ident(ident.to_string()),
        _ => ParameterBinding::Unsupported,
    }
}

fn parameter_attributes(cx: &LateContext<'_>, hir_id: hir::HirId) -> Vec<Attribute> {
    cx.tcx
        .hir_attrs(hir_id)
        .iter()
        .filter_map(attribute_from_hir)
        .collect()
}

fn attribute_from_hir(attr: &hir::Attribute) -> Option<Attribute> {
    Some(Attribute::new(attribute_path(attr)?, attribute_kind(attr)))
}

fn attribute_path(attr: &hir::Attribute) -> Option<AttributePath> {
    let hir::Attribute::Unparsed(_) = attr else {
        return None;
    };

    let mut names = attr.path().into_iter().map(|symbol| symbol.to_string());
    let first = names.next()?;
    Some(AttributePath::new(std::iter::once(first).chain(names)))
}

fn attribute_kind(attr: &hir::Attribute) -> AttributeKind {
    match attribute_style(attr) {
        AttrStyle::Inner => AttributeKind::Inner,
        AttrStyle::Outer => AttributeKind::Outer,
    }
}

fn attribute_style(attr: &hir::Attribute) -> AttrStyle {
    match attr {
        hir::Attribute::Unparsed(item) => item.style,
        hir::Attribute::Parsed(HirAttributeKind::DocComment { style, .. }) => *style,
        hir::Attribute::Parsed(_) => AttrStyle::Outer,
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
