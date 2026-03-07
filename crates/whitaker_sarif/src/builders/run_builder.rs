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
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_sarif::RunBuilder;
    ///
    /// let run = RunBuilder::new("tool", "1.0")
    ///     .with_information_uri("https://example.com")
    ///     .build();
    /// assert_eq!(run.tool.driver.information_uri.as_deref(), Some("https://example.com"));
    /// ```
    #[must_use]
    pub fn with_information_uri(mut self, uri: impl Into<String>) -> Self {
        self.information_uri = Some(uri.into());
        self
    }

    /// Appends rules to the tool driver.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_sarif::{RunBuilder, all_rules};
    ///
    /// let run = RunBuilder::new("tool", "1.0")
    ///     .with_rules(all_rules())
    ///     .build();
    /// assert_eq!(run.tool.driver.rules.len(), 3);
    /// ```
    #[must_use]
    pub fn with_rules(mut self, rules: Vec<ReportingDescriptor>) -> Self {
        self.rules.extend(rules);
        self
    }

    /// Appends a result to the run.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_sarif::{RunBuilder, ResultBuilder};
    ///
    /// let result = ResultBuilder::new("WHK001")
    ///     .with_message("clone")
    ///     .build()
    ///     .expect("valid result");
    /// let run = RunBuilder::new("tool", "1.0").with_result(result).build();
    /// assert_eq!(run.results.len(), 1);
    /// ```
    #[must_use]
    pub fn with_result(mut self, result: SarifResult) -> Self {
        self.results.push(result);
        self
    }

    /// Appends an invocation record.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_sarif::{RunBuilder, Invocation};
    ///
    /// let run = RunBuilder::new("tool", "1.0")
    ///     .with_invocation(Invocation {
    ///         execution_successful: true,
    ///         command_line: None,
    ///     })
    ///     .build();
    /// assert_eq!(run.invocations.len(), 1);
    /// ```
    #[must_use]
    pub fn with_invocation(mut self, invocation: Invocation) -> Self {
        self.invocations.push(invocation);
        self
    }

    /// Appends an artifact reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_sarif::{RunBuilder, Artifact, ArtifactLocation};
    ///
    /// let run = RunBuilder::new("tool", "1.0")
    ///     .with_artifact(Artifact {
    ///         location: ArtifactLocation {
    ///             uri: "src/main.rs".into(),
    ///             uri_base_id: None,
    ///         },
    ///         mime_type: Some("text/x-rust".into()),
    ///     })
    ///     .build();
    /// assert_eq!(run.artifacts.len(), 1);
    /// ```
    #[must_use]
    pub fn with_artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Consumes the builder and produces a [`Run`].
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_sarif::RunBuilder;
    ///
    /// let run = RunBuilder::new("tool", "1.0").build();
    /// assert_eq!(run.tool.driver.name, "tool");
    /// assert_eq!(run.tool.driver.version.as_deref(), Some("1.0"));
    /// assert!(run.results.is_empty());
    /// ```
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
    //! Unit tests for [`RunBuilder`] construction and method chaining.

    use super::*;
    use crate::rules::all_rules;
    use rstest::{fixture, rstest};

    #[fixture]
    fn builder() -> RunBuilder {
        RunBuilder::new("tool", "1.0")
    }

    #[rstest]
    fn builds_run_with_tool(builder: RunBuilder) {
        let run = builder.build();
        assert_eq!(run.tool.driver.name, "tool");
        assert_eq!(run.tool.driver.version.as_deref(), Some("1.0"));
    }

    #[rstest]
    fn builds_run_with_rules(builder: RunBuilder) {
        let run = builder.with_rules(all_rules()).build();
        assert_eq!(run.tool.driver.rules.len(), 3);
    }

    #[rstest]
    fn builds_run_with_chained_rules(builder: RunBuilder) {
        let rules_a = vec![crate::rules::whk001_rule()];
        let rules_b = vec![crate::rules::whk002_rule(), crate::rules::whk003_rule()];
        let run = builder.with_rules(rules_a).with_rules(rules_b).build();
        assert_eq!(run.tool.driver.rules.len(), 3);
    }

    #[rstest]
    fn builds_run_with_invocation(builder: RunBuilder) {
        let run = builder
            .with_invocation(Invocation {
                execution_successful: true,
                command_line: None,
            })
            .build();
        assert_eq!(run.invocations.len(), 1);
    }
}
