//! Builder for [`SarifResult`] objects.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::{Result, SarifError};
use crate::model::location::{Location, RelatedLocation};
use crate::model::result::{Level, Message, SarifResult};

/// Fluent builder for constructing a [`SarifResult`].
///
/// `rule_id` and `message` are required; calling [`build`](Self::build)
/// without them returns an error.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{ResultBuilder, Level};
///
/// let result = ResultBuilder::new("WHK001")
///     .with_message("Type-1 clone detected")
///     .with_level(Level::Warning)
///     .build()
///     .expect("build result");
///
/// assert_eq!(result.rule_id, "WHK001");
/// ```
#[derive(Debug, Clone, Default)]
pub struct ResultBuilder {
    rule_id: Option<String>,
    level: Level,
    message: Option<String>,
    locations: Vec<Location>,
    related_locations: Vec<RelatedLocation>,
    partial_fingerprints: HashMap<String, String>,
    properties: Option<Value>,
    baseline_state: Option<String>,
}

impl ResultBuilder {
    /// Creates a builder for a result with the given rule identifier.
    #[must_use]
    pub fn new(rule_id: impl Into<String>) -> Self {
        Self {
            rule_id: Some(rule_id.into()),
            ..Self::default()
        }
    }

    /// Sets the severity level.
    #[must_use]
    pub fn with_level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Sets the human-readable message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Appends a primary location.
    #[must_use]
    pub fn with_location(mut self, location: Location) -> Self {
        self.locations.push(location);
        self
    }

    /// Replaces all primary locations.
    #[must_use]
    pub fn with_locations(mut self, locations: Vec<Location>) -> Self {
        self.locations = locations;
        self
    }

    /// Appends a related location.
    #[must_use]
    pub fn with_related_location(mut self, rl: RelatedLocation) -> Self {
        self.related_locations.push(rl);
        self
    }

    /// Adds a partial fingerprint entry.
    #[must_use]
    pub fn with_fingerprint(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.partial_fingerprints.insert(key.into(), value.into());
        self
    }

    /// Sets the tool-specific properties.
    #[must_use]
    pub fn with_properties(mut self, properties: Value) -> Self {
        self.properties = Some(properties);
        self
    }

    /// Sets the baseline comparison state.
    #[must_use]
    pub fn with_baseline_state(mut self, state: impl Into<String>) -> Self {
        self.baseline_state = Some(state.into());
        self
    }

    /// Consumes the builder and produces a [`SarifResult`].
    ///
    /// # Errors
    ///
    /// Returns [`SarifError::MissingField`] if `rule_id` or `message` was not
    /// set.
    pub fn build(self) -> Result<SarifResult> {
        let rule_id = self
            .rule_id
            .ok_or_else(|| SarifError::MissingField("rule_id".into()))?;
        let message_text = self
            .message
            .ok_or_else(|| SarifError::MissingField("message".into()))?;

        Ok(SarifResult {
            rule_id,
            level: self.level,
            message: Message { text: message_text },
            locations: self.locations,
            related_locations: self.related_locations,
            partial_fingerprints: self.partial_fingerprints,
            properties: self.properties,
            baseline_state: self.baseline_state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("WHK001", "msg", Level::Warning)]
    #[case("WHK002", "clone found", Level::Note)]
    #[case("WHK003", "match", Level::Error)]
    fn builds_result_with_level(#[case] rule: &str, #[case] msg: &str, #[case] level: Level) {
        match ResultBuilder::new(rule)
            .with_message(msg)
            .with_level(level)
            .build()
        {
            Ok(result) => {
                assert_eq!(result.rule_id, rule);
                assert_eq!(result.message.text, msg);
                assert_eq!(result.level, level);
            }
            Err(e) => panic!("failed to build result: {e}"),
        }
    }

    #[test]
    fn missing_message_returns_error() {
        let result = ResultBuilder::new("WHK001").build();
        assert!(result.is_err());
    }

    #[test]
    fn missing_rule_id_returns_error() {
        let result = ResultBuilder::default().with_message("msg").build();
        assert!(result.is_err());
    }

    #[test]
    fn adds_fingerprints() {
        match ResultBuilder::new("WHK001")
            .with_message("msg")
            .with_fingerprint("whitakerFragment", "abc123")
            .build()
        {
            Ok(r) => {
                assert_eq!(
                    r.partial_fingerprints
                        .get("whitakerFragment")
                        .map(String::as_str),
                    Some("abc123")
                );
            }
            Err(e) => panic!("failed to build: {e}"),
        }
    }
}
