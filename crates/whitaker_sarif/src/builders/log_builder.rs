//! Builder for [`SarifLog`] objects.

use crate::model::log::{SARIF_SCHEMA, SARIF_VERSION, SarifLog};
use crate::model::run::Run;

/// Fluent builder for constructing a [`SarifLog`].
///
/// # Examples
///
/// ```
/// use whitaker_sarif::SarifLogBuilder;
///
/// let log = SarifLogBuilder::new().build();
/// assert_eq!(log.version, "2.1.0");
/// ```
#[derive(Debug, Clone)]
pub struct SarifLogBuilder {
    schema: String,
    version: String,
    runs: Vec<Run>,
}

impl SarifLogBuilder {
    /// Creates a builder pre-populated with the SARIF 2.1.0 schema and
    /// version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema: SARIF_SCHEMA.into(),
            version: SARIF_VERSION.into(),
            runs: Vec::new(),
        }
    }

    /// Appends a run to the log.
    #[must_use]
    pub fn with_run(mut self, run: Run) -> Self {
        self.runs.push(run);
        self
    }

    /// Consumes the builder and produces a [`SarifLog`].
    #[must_use]
    pub fn build(self) -> SarifLog {
        SarifLog {
            schema: self.schema,
            version: self.version,
            runs: self.runs,
        }
    }
}

impl Default for SarifLogBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::run::{Tool, ToolComponent};

    #[test]
    fn builds_empty_log() {
        let log = SarifLogBuilder::new().build();
        assert_eq!(log.version, "2.1.0");
        assert!(log.runs.is_empty());
    }

    #[test]
    fn builds_log_with_run() {
        let run = Run {
            tool: Tool {
                driver: ToolComponent {
                    name: "test".into(),
                    version: None,
                    information_uri: None,
                    rules: Vec::new(),
                },
            },
            invocations: Vec::new(),
            results: Vec::new(),
            artifacts: Vec::new(),
        };
        let log = SarifLogBuilder::new().with_run(run).build();
        assert_eq!(log.runs.len(), 1);
    }
}
