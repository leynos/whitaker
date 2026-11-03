//! Guards against the nextest overrides for dylint UI harnesses silently
//! missing or shadowing a harness.
//!
//! The `serial-dylint-ui` test-group in `.config/nextest.toml` serializes all
//! dylint UI tests so they do not race on the shared target directory or on
//! Windows temporary-file handles. If a lint crate adds a UI test whose
//! fully-qualified name does not match the filter, the test runs without
//! serialization or retries, causing flaky Windows CI failures.
//!
//! A timeout override can fail to reach a harness in two ways. A filter that
//! names individual tests must name every nested-Cargo harness in the package,
//! and a name list quietly dropped later ones back to the 300 s base, where
//! they were killed at 300.006 s. A broader override listed earlier in the
//! profile also shadows a later, more specific one, because nextest resolves
//! each setting from the *first* matching override that configures it.
//!
//! `no_unwrap_or_else_panic` is the crate that hit both, so it is the one this
//! test pins. Its UI module lives inline in `src/lib_ui_tests.rs`, which is
//! why the filter has to reach the *package's own* binary rather than a
//! `tests/ui.rs` integration binary named `ui`. The three name types below
//! make a swapped pair of `&str` arguments to `mentions` unrepresentable.

use std::{fmt, fs, path::Path};

use rstest::{fixture, rstest};
use toml::Value;

/// A nextest profile name such as `default` or `ci`.
#[derive(Clone, Copy, Debug)]
struct ProfileName<'a>(&'a str);

/// A cargo package name, for example `no_unwrap_or_else_panic`.
#[derive(Clone, Copy, Debug)]
struct PackageName<'a>(&'a str);

impl fmt::Display for PackageName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.0) }
}

/// A nextest filter expression such as `package(foo) & binary(foo)`.
#[derive(Clone, Copy, Debug)]
struct FilterExpression<'a>(&'a str);

impl FilterExpression<'_> {
    /// Returns whether the expression positively selects `package`; negated
    /// clauses exclude a crate rather than govern it, so they are stripped.
    #[must_use]
    fn mentions(&self, package: PackageName<'_>) -> bool {
        let negations = [
            format!("not package({package})"),
            format!("not binary({package})"),
        ];
        let positive = negations.iter().fold(self.0.to_owned(), |text, negation| {
            text.replace(negation.as_str(), "")
        });

        positive.contains(&format!("package({package})"))
            || positive.contains(&format!("binary({package})"))
    }
}

/// The crate whose nested-Cargo harnesses timed out.
const HARNESS_PACKAGE: PackageName<'static> = PackageName("no_unwrap_or_else_panic");

/// The test-group every dylint UI harness must be serialized into.
const SERIAL_UI_GROUP: &str = "serial-dylint-ui";

/// The minimum period that counts as a build-sized allowance, in seconds.
///
/// Nested Cargo builds queue behind shared compiler admission and package
/// caches; twenty minutes is the smallest budget the repository has needed.
/// Anything shorter is a hang detector, not an allowance.
const BUILD_SLOW_TIMEOUT_SECS: u64 = 20 * 60;

/// Parses `.config/nextest.toml` into a [`Value`].
fn load_nextest_config() -> Value {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".config/nextest.toml");
    let contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
    toml::from_str(&contents)
        .unwrap_or_else(|err| panic!(".config/nextest.toml should parse as TOML: {err}"))
}

/// Returns the ordered override tables for `profile`.
fn profile_overrides<'a>(config: &'a Value, profile: ProfileName<'_>) -> &'a [Value] {
    let Some(overrides) = config
        .get("profile")
        .and_then(|p| p.get(profile.0))
        .and_then(|d| d.get("overrides"))
        .and_then(Value::as_array)
    else {
        panic!("profile.{}: overrides should be an array", profile.0);
    };

    overrides.as_slice()
}

/// Returns the `serial-dylint-ui` override table from the default profile.
fn find_serial_dylint_ui_override(config: &Value) -> &Value {
    profile_overrides(config, ProfileName("default"))
        .iter()
        .find(|o| {
            o.get("test-group")
                .and_then(Value::as_str)
                .is_some_and(|group| group == SERIAL_UI_GROUP)
        })
        .unwrap_or_else(|| panic!("serial-dylint-ui override should exist"))
}

/// Returns the filter expression from the `serial-dylint-ui` override.
fn extract_filter(ui_override: &Value) -> FilterExpression<'_> {
    filter_of(ui_override)
        .unwrap_or_else(|| panic!("serial-dylint-ui override should have a filter string"))
}

/// Reads an override's `filter` string, if it has one.
fn filter_of(override_table: &Value) -> Option<FilterExpression<'_>> {
    override_table
        .get("filter")
        .and_then(Value::as_str)
        .map(FilterExpression)
}

/// Reads the `period` of an override's `slow-timeout`, in seconds.
///
/// Everything under test lives in a profile whose base `slow-timeout` is a
/// table; a bare string or a missing `slow-timeout` is a configuration bug
/// rather than something to reinterpret.
fn slow_timeout_period_secs(overrides: &Value) -> Option<u64> {
    let period = overrides.get("slow-timeout")?.get("period")?.as_str()?;
    Some(parse_duration_secs(period))
}

/// Parses the nextest integer-with-suffix duration form, e.g. `"75m"`.
///
/// The suffixes are exactly the three below, each mapping to seconds; an
/// unknown one is a configuration bug.
fn parse_duration_secs(period: &str) -> u64 {
    const SUFFIXES: [(char, u64); 3] = [('s', 1), ('m', 60), ('h', 60 * 60)];

    let Some((suffix, multiplier)) = SUFFIXES
        .iter()
        .find(|(suffix, _)| period.ends_with(*suffix))
    else {
        panic!("slow-timeout period should end in s, m, or h; found {period:?}");
    };

    let digits = period
        .strip_suffix(*suffix)
        .unwrap_or_else(|| panic!("period {period:?} should end in {suffix:?}"));

    digits
        .parse::<u64>()
        .unwrap_or_else(|err| panic!("slow-timeout period {period:?} should be an integer: {err}"))
        * multiplier
}

/// A test function found in the harness crate's UI module.
struct UiTest {
    /// The test name nextest reports, e.g. `example_compiles_without_diagnostics`.
    name: String,
    /// Whether the body rebuilds the lint through nested Cargo, which queues
    /// behind shared compiler admission and needs a build-sized allowance.
    builds_through_nested_cargo: bool,
}

/// Returns the names of the crate's tests that rebuild through nested Cargo.
fn nested_cargo_harness_names() -> Vec<String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join(HARNESS_PACKAGE.0)
        .join("src/lib_ui_tests.rs");
    let contents = fs::read_to_string(&source)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", source.display()));

    ui_tests(&contents)
        .into_iter()
        .filter(|test| test.builds_through_nested_cargo)
        .map(|test| test.name)
        .collect()
}

/// Returns whether a test body runs a nested Cargo build.
///
/// Every marker is the repository's own; see `crates/*/src/lib_ui_tests.rs`
/// and `crates/rstest_helper_should_be_fixture/tests/ui.rs` for the shapes.
fn builds_through_nested_cargo(body: &str) -> bool {
    [
        "run_example_under_test_harness",
        "run_fixture_under_test_harness",
        "run_fixture_harness_test",
        "ExampleHarness",
        "TestCases::new",
    ]
    .iter()
    .any(|marker| body.contains(marker))
}

/// Collects every `#[test]` and `#[rstest]` function declared in `source`.
///
/// `rstest` cases share one function name, and nextest reports that name, so
/// the function names are exactly the unit the filter must reach. Attributes
/// are accumulated rather than parsed, because they routinely span several
/// lines; the only question is whether the block before a top-level `fn`
/// contains a test attribute.
fn ui_tests(source: &str) -> Vec<UiTest> {
    let lines: Vec<&str> = source.lines().collect();
    let mut tests = Vec::new();
    let mut pending = String::new();
    let mut index = 0;

    while let Some(line) = lines.get(index) {
        // Only a column-zero `fn` is a top-level item; helpers inside `impl`
        // blocks and modules are indented and are never test entrypoints.
        if let Some(rest) = line.strip_prefix("fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let (body, next) = function_body(&lines, index);

            if pending.contains("#[test]") || pending.contains("#[rstest]") {
                tests.push(UiTest {
                    name,
                    builds_through_nested_cargo: builds_through_nested_cargo(&body),
                });
            }

            pending.clear();
            index = next;
            continue;
        }

        pending.push_str(line);
        pending.push('\n');
        index += 1;
    }

    tests
}

/// Returns the text of the function starting at `start` and the next index.
/// The body runs to the first line that is `}` in column zero, which is where
/// a top-level item in this repository ends.
fn function_body(lines: &[&str], start: usize) -> (String, usize) {
    let mut body = String::new();
    let mut index = start;

    while let Some(line) = lines.get(index) {
        body.push_str(line);
        body.push('\n');
        if index > start && *line == "}" {
            return (body, index + 1);
        }
        index += 1;
    }

    (body, index)
}

/// Fixture providing the `serial-dylint-ui` override from nextest config.
#[fixture]
fn serial_dylint_ui_override() -> Value {
    let config = load_nextest_config();
    find_serial_dylint_ui_override(&config).clone()
}

#[rstest]
fn serial_dylint_ui_filter_captures_integration_ui_binaries(serial_dylint_ui_override: Value) {
    let filter = extract_filter(&serial_dylint_ui_override);

    // The integration-test pattern produces binary=ui, test=ui, so the filter
    // must match it; `test(ui::ui)` misses because the reported name is `ui`.
    assert!(
        filter.0.contains("(binary(ui) & test(=ui))"),
        "the serial-dylint-ui filter must contain `(binary(ui) & test(=ui))` to capture \
         integration test binaries named `ui` with a top-level `fn ui()`; found filter: {}",
        filter.0
    );
}

#[rstest]
fn serial_dylint_ui_filter_has_retry_configuration(serial_dylint_ui_override: Value) {
    // On Windows, dylint_testing::initialize() can transiently fail with
    // "Access is denied (os error 5)" during NamedTempFile::persist().
    // Retries are essential for reliable CI on Windows.
    let Some(count) = serial_dylint_ui_override
        .get("retries")
        .and_then(|retries| retries.get("count"))
        .and_then(Value::as_integer)
    else {
        panic!("serial-dylint-ui override must configure retries.count for Windows CI");
    };

    assert!(
        count >= 1,
        "retries.count must be at least 1 for Windows CI reliability; found {count}"
    );
}

/// nextest resolves a setting from the first override that matches a test and
/// configures that setting, so the governing override for a harness is the
/// earliest one that sets `slow-timeout` and positively selects its package.
/// This is a textual containment check, not a full filter evaluation: it
/// cannot prove a clause matches a *particular* test name, but it does catch
/// a name list that reaches only some harnesses, and an earlier broad clause
/// that wins the setting.
#[rstest]
#[case::default(ProfileName("default"))]
#[case::ci(ProfileName("ci"))]
fn harness_allowances_are_governed_by_a_whole_package_override(#[case] profile: ProfileName<'_>) {
    let config = load_nextest_config();
    let overrides = profile_overrides(&config, profile);

    let Some((index, governing)) = overrides.iter().enumerate().find(|(_, over)| {
        over.get("slow-timeout").is_some()
            && filter_of(over).is_some_and(|filter| filter.mentions(HARNESS_PACKAGE))
    }) else {
        panic!(
            "profile.{}: no override with a slow-timeout positively selects {HARNESS_PACKAGE}, so \
             its nested-Cargo harnesses fall to the base bound",
            profile.0
        );
    };

    let Some(filter) = filter_of(governing) else {
        panic!(
            "profile.{}.overrides[{index}] should have a filter",
            profile.0
        );
    };

    // A name list is what rotted: it silently stopped reaching harnesses that
    // were added later. Selecting the package's own binary covers every test
    // in it without an enumerable list.
    assert!(
        filter.0.contains(&format!("binary({HARNESS_PACKAGE})")),
        "profile.{}.overrides[{index}] governs {HARNESS_PACKAGE} by name list rather than the \
         whole package binary, so a new harness would silently fall to the base bound; found \
         filter: {}",
        profile.0,
        filter.0
    );

    let period = slow_timeout_period_secs(governing).unwrap_or_else(|| {
        panic!(
            "profile.{}.overrides[{index}] should set slow-timeout.period",
            profile.0
        )
    });
    assert!(
        period >= BUILD_SLOW_TIMEOUT_SECS,
        "profile.{}.overrides[{index}] gives {HARNESS_PACKAGE} a {period}s slow-timeout, below \
         the {BUILD_SLOW_TIMEOUT_SECS}s build-sized allowance. nextest uses the *first* matching \
         override that configures a setting, so an earlier broad clause shadows a later one.",
        profile.0
    );

    let group = governing.get("test-group").and_then(Value::as_str);
    assert_eq!(
        group,
        Some(SERIAL_UI_GROUP),
        "profile.{}.overrides[{index}] governs {HARNESS_PACKAGE} but its test-group is {group:?}; \
         nested-Cargo UI harnesses must be serialized in {SERIAL_UI_GROUP}",
        profile.0
    );
}

#[rstest]
fn the_harness_crawl_finds_the_known_nested_cargo_harnesses() {
    // A regression test that silently stops finding the harnesses it is about
    // is worse than no test: it reports a clean bill of health for a filter
    // nobody checked. Pin the harnesses that have actually timed out so the
    // crawl cannot hollow out.
    let found = nested_cargo_harness_names();

    for expected in [
        "example_compiles_under_test_harness",
        "hand_written_test_companion_does_not_exempt_parent_function",
        "rstest_unwrap_outside_tests_still_fails_in_non_harness_code",
        "rstest_empty_companion_does_not_exempt_parent_function",
        "aliased_test_crate_non_companion_does_not_exempt_parent_function",
    ] {
        assert!(
            found.iter().any(|name| name == expected),
            "the crawl should find `{expected}` among {HARNESS_PACKAGE}'s nested-Cargo harnesses; \
             found {found:?}"
        );
    }

    // The crate also has a cheap `fn ui()` fixture test and helpers inside
    // `impl` blocks; neither rebuilds through Cargo, so the crawl must not
    // report them as harnesses.
    assert!(
        !found
            .iter()
            .any(|name| name == "ui" || name == "run_fixture"),
        "the crawl should report only nested-Cargo harnesses; found {found:?}"
    );
}
