//! Shared test support utilities for behaviour-driven suites.
//!
//! Exposes the `locale` helpers (for example `StepLocale`) that parse locale
//! parameters in BDD steps so scenarios can feed consistent values into the
//! configuration and resolution flow. Reach for these helpers whenever a test
//! needs to normalise locale input before exercising the i18n layer.
pub mod locale;
