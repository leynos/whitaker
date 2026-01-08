//! Lint crate enforcing capability-based filesystem access by forbidding
//! `std::fs` operations.

use crate::diagnostics::emit_diagnostic;
use crate::usage::{
    StdFsUsage, UsageCategory, classify_def_id, classify_qpath, classify_res, label_is_std_fs,
};
use common::i18n::Localizer;
use common::i18n::get_localizer_for_lint;
use log::{info, warn};
use rustc_hir as hir;
use rustc_hir::AmbigArg;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::{Span, sym};
use serde::Deserialize;
use whitaker::SharedConfig;

const LINT_NAME: &str = "no_std_fs_operations";

/// Configuration for the `no_std_fs_operations` lint.
///
/// # TOML Configuration
///
/// In `dylint.toml` at your workspace root:
///
/// ```toml
/// [no_std_fs_operations]
/// excluded_crates = ["my_cli_app", "test_utilities"]
/// ```
///
/// Use Rust crate names (underscores), not Cargo package names (hyphens).
///
/// # Strict Validation
///
/// This configuration uses `deny_unknown_fields`, meaning any unrecognised
/// key (such as a typo like `excluded_crate` instead of `excluded_crates`)
/// will cause configuration parsing to fail. When parsing fails, the lint
/// falls back to defaults and logs a warning. If exclusions don't work as
/// expected, check the logs for parse errors.
///
/// # Example
///
/// ```
/// use serde::Deserialize;
///
/// #[derive(Clone, Debug, Default, Deserialize)]
/// #[serde(default, deny_unknown_fields)]
/// struct NoStdFsConfig {
///     excluded_crates: Vec<String>,
/// }
///
/// let toml_str = r#"excluded_crates = ["my_cli_app"]"#;
/// let config: NoStdFsConfig = toml::from_str(toml_str).expect("valid TOML");
/// assert_eq!(config.excluded_crates, vec!["my_cli_app"]);
/// ```
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NoStdFsConfig {
    /// Crate names excluded from the lint. These crates are allowed to use
    /// `std::fs` operations without triggering diagnostics.
    pub excluded_crates: Vec<String>,
}

impl NoStdFsConfig {
    /// Check if the given crate name is excluded from the lint.
    ///
    /// Returns `true` if `crate_name` appears in the `excluded_crates` list.
    ///
    /// # Example
    ///
    /// ```
    /// # use serde::Deserialize;
    /// #[derive(Default, Deserialize)]
    /// struct NoStdFsConfig {
    ///     excluded_crates: Vec<String>,
    /// }
    ///
    /// impl NoStdFsConfig {
    ///     fn is_excluded(&self, crate_name: &str) -> bool {
    ///         self.excluded_crates.iter().any(|c| c == crate_name)
    ///     }
    /// }
    ///
    /// let config = NoStdFsConfig {
    ///     excluded_crates: vec!["my_cli".to_owned(), "test_utils".to_owned()],
    /// };
    ///
    /// assert!(config.is_excluded("my_cli"));
    /// assert!(!config.is_excluded("other_crate"));
    /// ```
    #[must_use]
    pub fn is_excluded(&self, crate_name: &str) -> bool {
        self.excluded_crates.iter().any(|c| c == crate_name)
    }
}

pub struct NoStdFsOperations {
    localizer: Localizer,
    excluded: bool,
}

impl Default for NoStdFsOperations {
    fn default() -> Self {
        Self {
            localizer: Localizer::new(None),
            excluded: false,
        }
    }
}

dylint_linting::impl_late_lint! {
    pub NO_STD_FS_OPERATIONS,
    Deny,
    "std::fs operations bypass Whitaker's capability-based filesystem policy",
    NoStdFsOperations::default()
}

impl<'tcx> LateLintPass<'tcx> for NoStdFsOperations {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());

        let config = load_configuration();
        let crate_name_sym = cx.tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
        let crate_name = crate_name_sym.as_str();

        self.excluded = config.is_excluded(crate_name);

        if self.excluded {
            info!(
                target: LINT_NAME,
                "crate `{crate_name}` is excluded from no_std_fs_operations lint"
            );
        }
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Use(path, ..) = item.kind {
            for res in path.res.present_items() {
                let usage = classify_res(cx, res, UsageCategory::Import);
                self.emit_optional(cx, path.span, usage);
            }
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        match &expr.kind {
            hir::ExprKind::Path(qpath) => {
                let usage = classify_qpath(cx, qpath, expr.hir_id, UsageCategory::Call);
                self.emit_optional(cx, expr.span, usage);
            }
            hir::ExprKind::Struct(qpath, ..) => {
                let usage = classify_qpath(cx, qpath, expr.hir_id, UsageCategory::Call);
                self.emit_optional(cx, expr.span, usage);
            }
            hir::ExprKind::MethodCall(segment, receiver, ..) => {
                let mut usage = cx
                    .typeck_results()
                    .type_dependent_def_id(expr.hir_id)
                    .and_then(|def_id| classify_def_id(cx, def_id, UsageCategory::Call));

                if usage.is_none() {
                    usage = self.receiver_usage_for_method(cx, receiver, segment.ident.as_str());
                }

                self.emit_optional(cx, expr.span, usage);
            }
            _ => {}
        }
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx hir::Ty<'tcx, AmbigArg>) {
        if let hir::TyKind::Path(qpath) = &ty.kind {
            let usage = classify_qpath(cx, qpath, ty.hir_id, UsageCategory::Type);
            self.emit_optional(cx, ty.span, usage);
        }
    }
}

impl NoStdFsOperations {
    fn emit_optional(&self, cx: &LateContext<'_>, span: Span, usage: Option<StdFsUsage>) {
        if self.excluded {
            return;
        }
        if let Some(usage) = usage {
            self.emit(cx, span, usage);
        }
    }

    fn emit(&self, cx: &LateContext<'_>, span: Span, usage: StdFsUsage) {
        emit_diagnostic(cx, span, usage, &self.localizer);
    }

    fn receiver_usage_for_method(
        &self,
        cx: &LateContext<'_>,
        receiver: &hir::Expr<'_>,
        method: &str,
    ) -> Option<StdFsUsage> {
        let ty = cx.typeck_results().expr_ty(receiver).peel_refs();

        let ty::Adt(adt, _) = ty.kind() else {
            return None;
        };

        let def_id = adt.did();
        if cx.tcx.crate_name(def_id.krate) != sym::std {
            return None;
        }

        let label = cx.tcx.def_path_str(def_id);
        if !label_is_std_fs(&label) {
            return None;
        }

        let operation = format!("{label}::{method}");
        Some(StdFsUsage::new(operation, UsageCategory::Call))
    }
}

/// Trait for loading lint configuration, enabling dependency injection for tests.
///
/// # Return Values
///
/// - `Ok(Some(config))` - Configuration section found and parsed successfully
/// - `Ok(None)` - No configuration file or section present
/// - `Err(...)` - Configuration file exists but parsing failed
///
/// # Example Usage (in tests)
///
/// ```text
/// let mut mock = MockConfigReader::new();
/// mock.expect_read_config()
///     .returning(|_| Ok(Some(NoStdFsConfig {
///         excluded_crates: vec!["my_crate".to_owned()],
///     })));
///
/// let config = load_configuration_with_reader(&mock);
/// assert!(config.is_excluded("my_crate"));
/// ```
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ConfigReader {
    /// Read configuration for the given lint name.
    ///
    /// Returns `Ok(Some(config))` if found, `Ok(None)` if not present, or
    /// `Err` if parsing fails.
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<NoStdFsConfig>, Box<dyn std::error::Error>>;
}

/// Production implementation that reads from `dylint.toml` via `dylint_linting::config`.
///
/// This reader delegates to `dylint_linting::config` to locate and parse
/// the `dylint.toml` configuration file in the workspace root.
pub(crate) struct DylintConfigReader;

impl ConfigReader for DylintConfigReader {
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<NoStdFsConfig>, Box<dyn std::error::Error>> {
        dylint_linting::config::<NoStdFsConfig>(lint_name).map_err(|e| Box::new(e) as _)
    }
}

/// Load lint configuration using the provided reader.
///
/// Returns the default configuration when:
/// - No configuration file exists
/// - No `[no_std_fs_operations]` section is present
/// - The configuration fails to parse (logged at `warn` level)
///
/// # Example Return Cases
///
/// ```text
/// // Case 1: Configuration present
/// reader.read_config("lint") => Ok(Some(config))
/// load_configuration_with_reader(&reader) => config
///
/// // Case 2: No configuration section
/// reader.read_config("lint") => Ok(None)
/// load_configuration_with_reader(&reader) => NoStdFsConfig::default()
///
/// // Case 3: Parse error
/// reader.read_config("lint") => Err(...)
/// load_configuration_with_reader(&reader) => NoStdFsConfig::default() + logs warning
/// ```
fn load_configuration_with_reader(reader: &dyn ConfigReader) -> NoStdFsConfig {
    match reader.read_config(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => NoStdFsConfig::default(),
        Err(error) => {
            warn!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            NoStdFsConfig::default()
        }
    }
}

/// Load lint configuration from `dylint.toml`.
///
/// Returns the default configuration when:
/// - No configuration file exists
/// - No `[no_std_fs_operations]` section is present
/// - The configuration fails to parse (logged at `warn` level)
fn load_configuration() -> NoStdFsConfig {
    load_configuration_with_reader(&DylintConfigReader)
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
