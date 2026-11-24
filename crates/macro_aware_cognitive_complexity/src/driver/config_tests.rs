//! Unit tests for configuration parsing and for the loading seam.
//!
//! The mocked reader covers the three outcomes the lint must distinguish:
//! absent configuration, a valid document, and an unreadable one. The last case
//! also asserts the warning, because an operator only learns about a mistyped
//! key from the log.

use std::{
    io,
    sync::{Mutex, Once},
};

use rstest::rstest;

use super::{Config, MockConfigReader, load_configuration_with_reader};
use crate::driver::LINT_NAME;

/// Verify supported configuration documents and the default limit.
#[rstest]
#[case("", 25)]
#[case("max_complexity = 7\n", 7)]
#[case("max_complexity = 0\n", 0)]
fn configuration_parses_supported_values(#[case] document: &str, #[case] expected: u64) {
    let config: Config = toml::from_str(document).expect("configuration should parse");
    assert_eq!(
        config.max_complexity, expected,
        "unexpected limit for document {document:?}"
    );
}

/// Verify malformed configuration documents are rejected rather than defaulted.
#[rstest]
#[case("unknown_field = 1\n")]
#[case("max_complexity = -1\n")]
#[case("max_complexity = \"seven\"\n")]
fn configuration_rejects_malformed_documents(#[case] document: &str) {
    assert!(
        toml::from_str::<Config>(document).is_err(),
        "document should be rejected: {document:?}"
    );
}

/// Verify an absent configuration section yields the default limit.
#[test]
fn load_configuration_defaults_when_absent() {
    let mut reader = MockConfigReader::new();
    reader.expect_read_config().returning(|_| Ok(None));

    assert_eq!(
        load_configuration_with_reader(&reader),
        Config::default(),
        "an absent section must use defaults"
    );
}

/// Verify a parsed configuration supplies the limit.
#[test]
fn load_configuration_uses_a_valid_document() {
    let mut reader = MockConfigReader::new();
    reader
        .expect_read_config()
        .returning(|_| Ok(Some(Config { max_complexity: 7 })));

    assert_eq!(
        load_configuration_with_reader(&reader).max_complexity,
        7,
        "a parsed document must supply the limit"
    );
}

/// Verify a parse failure returns defaults and warns the operator.
#[test]
fn load_configuration_warns_and_defaults_on_error() {
    let mut reader = MockConfigReader::new();
    reader
        .expect_read_config()
        .returning(|_| Err(parse_failure()));

    let (config, warnings) = capture_warnings(|| load_configuration_with_reader(&reader));

    assert_eq!(
        config,
        Config::default(),
        "a parse failure must use defaults"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains(LINT_NAME) && warning.contains("using defaults")),
        "expected a warning naming the lint, captured {warnings:?}"
    );
}

/// Build the error a reader returns for an unparsable document.
fn parse_failure() -> Box<dyn std::error::Error + Send + Sync + 'static> {
    Box::new(io::Error::other("parse error"))
}

/// Records the warning text emitted while `action` runs.
static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Installs the capturing logger exactly once per test binary.
static INSTALL_LOGGER: Once = Once::new();

/// Capturing logger that keeps only the text of warning records.
struct WarningLogger;

impl log::Log for WarningLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool { metadata.level() <= log::Level::Warn }

    fn log(&self, record: &log::Record<'_>) {
        if record.level() != log::Level::Warn {
            return;
        }
        if let Ok(mut warnings) = WARNINGS.lock() {
            warnings.push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

/// Run `action` and return its result with the warnings it emitted.
///
/// The logger is process-wide, so this helper serializes on the capture buffer:
/// it clears the buffer before running and takes the emitted text afterwards.
/// Only this test reads warnings, which keeps the shared state honest.
fn capture_warnings<T>(action: impl FnOnce() -> T) -> (T, Vec<String>) {
    static LOGGER: WarningLogger = WarningLogger;
    INSTALL_LOGGER.call_once(|| {
        log::set_logger(&LOGGER).expect("the capturing logger should install");
        log::set_max_level(log::LevelFilter::Warn);
    });

    if let Ok(mut warnings) = WARNINGS.lock() {
        warnings.clear();
    }

    let value = action();

    let warnings = WARNINGS
        .lock()
        .map(|warnings| warnings.clone())
        .unwrap_or_default();
    (value, warnings)
}
