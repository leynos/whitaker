//! Builder for [`Run`] objects.

use crate::model::descriptor::ReportingDescriptor;
use crate::model::result::SarifResult;
use crate::model::run::{Artifact, Invocation, Run, Tool, ToolComponent};

/// Fluent builder for constructing a [`Run`].
///
/// # Examples
///
/// ```
/// use whitaker_sarif::RunBuilder;
///
/// let run = RunBuilder::new("whitaker_clones_cli", "0.2.1").build();
/// assert_eq!(run.tool.driver.name, "whitaker_clones_cli");
/// ```
#[derive(Debug, Clone)]
pub struct RunBuilder {
    tool_name: String,
    tool_version: Option<String>,
    information_uri: Option<String>,
    rules: Vec<ReportingDescriptor>,
    invocations: Vec<Invocation>,
    results: Vec<SarifResult>,
    artifacts: Vec<Artifact>,
}

impl RunBuilder {
    /// Creates a builder for a run produced by the named tool.
    #[must_use]
    pub fn new(tool_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_version: Some(version.into()),
            information_uri: None,
            rules: Vec::new(),
            invocations: Vec::new(),
            results: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    /// Sets the tool information URI.
    #[must_use]
    pub fn with_information_uri(mut self, uri: impl Into<String>) -> Self {
        self.information_uri = Some(uri.into());
        self
    }

    /// Adds rules to the tool driver.
    #[must_use]
    pub fn with_rules(mut self, rules: Vec<ReportingDescriptor>) -> Self {
        self.rules = rules;
        self
    }

    /// Appends a result to the run.
    #[must_use]
    pub fn with_result(mut self, result: SarifResult) -> Self {
        self.results.push(result);
        self
    }

    /// Appends an invocation record.
    #[must_use]
    pub fn with_invocation(mut self, invocation: Invocation) -> Self {
        self.invocations.push(invocation);
        self
    }

    /// Appends an artifact reference.
    #[must_use]
    pub fn with_artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Consumes the builder and produces a [`Run`].
    #[must_use]
    pub fn build(self) -> Run {
        Run {
            tool: Tool {
                driver: ToolComponent {
                    name: self.tool_name,
                    version: self.tool_version,
                    information_uri: self.information_uri,
                    rules: self.rules,
                },
            },
            invocations: self.invocations,
            results: self.results,
            artifacts: self.artifacts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::all_rules;

    #[test]
    fn builds_run_with_tool() {
        let run = RunBuilder::new("tool", "1.0").build();
        assert_eq!(run.tool.driver.name, "tool");
        assert_eq!(run.tool.driver.version.as_deref(), Some("1.0"));
    }

    #[test]
    fn builds_run_with_rules() {
        let run = RunBuilder::new("tool", "1.0")
            .with_rules(all_rules())
            .build();
        assert_eq!(run.tool.driver.rules.len(), 3);
    }

    #[test]
    fn builds_run_with_invocation() {
        let run = RunBuilder::new("tool", "1.0")
            .with_invocation(Invocation {
                execution_successful: true,
                command_line: None,
            })
            .build();
        assert_eq!(run.invocations.len(), 1);
    }
}
