//! Zero-configuration RAII fixtures for integration tests.
//!
//! The [`TestCluster`] type simulates a disposable Postgres-style deployment
//! backed by a temporary directory. Builders validate identifiers, prevent
//! destructive bootstrap statements, and record every statement applied during
//! setup so tests can assert on preparatory work without connecting to a real
//! database.
//!
//! ```no_run
//! use rstest::rstest;
//! use whitaker::testing::cluster::{test_cluster, TestCluster};
//!
//! #[rstest]
//! fn runs_queries_against_cluster(test_cluster: TestCluster) {
//!     assert!(test_cluster.connection_uri().starts_with("postgresql://"));
//!     assert_eq!(test_cluster.database(), "whitaker_test");
//! }
//! ```
//!
//! ```
//! use whitaker::testing::cluster::TestCluster;
//!
//! let mut builder = TestCluster::builder();
//! builder.database("demo_db").port(15_500).bootstrap_statement("CREATE TABLE demo (id INT)");
//! let cluster = builder.build().expect("cluster should build");
//! assert_eq!(cluster.database(), "demo_db");
//! assert_eq!(cluster.executed_statements(), ["CREATE TABLE demo (id INT)"]);
//! ```

use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};
use rstest::fixture;
use tempfile::TempDir;
use thiserror::Error;

const DEFAULT_DATABASE: &str = "whitaker_test";
const DEFAULT_USERNAME: &str = "postgres";
const DEFAULT_PORT: u16 = 15_432;

/// Errors that can occur while preparing a [`TestCluster`].
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ClusterError {
    /// The simulated data directory could not be created.
    #[error("cluster workspace could not be created: {message}")]
    WorkspaceCreation { message: String },
    /// The temporary directory path was not valid UTF-8.
    #[error("cluster workspace path is not valid UTF-8: {path:?}")]
    NonUtf8Path { path: PathBuf },
    /// Database identifiers must be ASCII letters, digits, or underscores.
    #[error("database name must be ASCII alphanumeric with underscores: {provided}")]
    InvalidDatabaseName { provided: String },
    /// Usernames share the same validation as database identifiers.
    #[error("username must be ASCII alphanumeric with underscores: {provided}")]
    InvalidUsername { provided: String },
    /// Ports must avoid privileged ranges.
    #[error("port must fall between 1024 and 65535 inclusive: {provided}")]
    InvalidPort { provided: u16 },
    /// Bootstrap statements must contain characters once trimmed.
    #[error("bootstrap statements must not be empty")]
    EmptyBootstrapStatement,
    /// Destructive statements are rejected by default for safety.
    #[error("bootstrap statement contains destructive verbs: {statement}")]
    UnsafeBootstrapStatement { statement: String },
}

/// Disposable Postgres-style cluster for integration tests.
#[derive(Debug)]
pub struct TestCluster {
    _root: TempDir,
    data_dir: Utf8PathBuf,
    username: String,
    database: String,
    port: u16,
    executed_statements: Vec<String>,
}

impl TestCluster {
    /// Returns a builder pre-populated with safe defaults.
    #[must_use]
    pub fn builder() -> TestClusterBuilder {
        TestClusterBuilder::default()
    }

    /// Data directory backing the simulated cluster.
    #[must_use]
    pub fn data_directory(&self) -> &Utf8Path {
        self.data_dir.as_ref()
    }

    /// Username that the fixture exposes.
    #[must_use]
    pub const fn username(&self) -> &str {
        self.username.as_str()
    }

    /// Database name configured for the fixture.
    #[must_use]
    pub const fn database(&self) -> &str {
        self.database.as_str()
    }

    /// Port exposed via the connection URI.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Connection string suitable for clients that accept libpq URIs.
    #[must_use]
    pub fn connection_uri(&self) -> String {
        format!(
            "postgresql://{}@localhost:{}/{}",
            self.username, self.port, self.database,
        )
    }

    /// Statements applied while bootstrapping the cluster.
    #[must_use]
    pub fn executed_statements(&self) -> &[String] {
        &self.executed_statements
    }
}

/// Configures and constructs [`TestCluster`] instances.
#[derive(Clone, Debug)]
pub struct TestClusterBuilder {
    username: String,
    database: String,
    port: u16,
    bootstrap: Vec<String>,
    allow_destructive: bool,
}

impl Default for TestClusterBuilder {
    fn default() -> Self {
        Self {
            username: DEFAULT_USERNAME.to_owned(),
            database: DEFAULT_DATABASE.to_owned(),
            port: DEFAULT_PORT,
            bootstrap: Vec::new(),
            allow_destructive: false,
        }
    }
}

impl TestClusterBuilder {
    /// Overrides the simulated username used by the fixture.
    pub fn username(&mut self, username: impl Into<String>) -> &mut Self {
        self.username = username.into();
        self
    }

    /// Overrides the database name exposed by the fixture.
    pub fn database(&mut self, database: impl Into<String>) -> &mut Self {
        self.database = database.into();
        self
    }

    /// Overrides the TCP port recorded in the connection URI.
    #[expect(
        clippy::missing_const_for_fn,
        reason = "builder mutators rely on heap-backed state that is not const"
    )]
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    /// Appends a bootstrap statement applied during cluster creation.
    pub fn bootstrap_statement(&mut self, statement: impl Into<String>) -> &mut Self {
        self.bootstrap.push(statement.into());
        self
    }

    /// Allows destructive statements such as `DROP DATABASE` during bootstrap.
    #[expect(
        clippy::missing_const_for_fn,
        reason = "builder mutators rely on heap-backed state that is not const"
    )]
    pub fn allow_destructive_bootstrap(&mut self, allow: bool) -> &mut Self {
        self.allow_destructive = allow;
        self
    }

    /// Builds the [`TestCluster`], validating identifiers and bootstrap input.
    ///
    /// # Errors
    ///
    /// Returns [`ClusterError`] when identifiers are invalid, the port falls
    /// outside the accepted range, bootstrap statements are empty or
    /// destructive, or when the temporary data directory cannot be created.
    pub fn build(&self) -> Result<TestCluster, ClusterError> {
        validate_identifier(&self.database)
            .map_err(|provided| ClusterError::InvalidDatabaseName { provided })?;
        validate_identifier(&self.username)
            .map_err(|provided| ClusterError::InvalidUsername { provided })?;
        validate_port(self.port)?;

        let root = TempDir::new().map_err(|error| ClusterError::WorkspaceCreation {
            message: error.to_string(),
        })?;
        let data_dir = Utf8PathBuf::from_path_buf(root.path().to_path_buf())
            .map_err(|path| ClusterError::NonUtf8Path { path })?;

        let mut executed_statements = Vec::with_capacity(self.bootstrap.len());
        for statement in &self.bootstrap {
            let trimmed = statement.trim();
            if trimmed.is_empty() {
                return Err(ClusterError::EmptyBootstrapStatement);
            }
            guard_statement(trimmed, self.allow_destructive)?;
            executed_statements.push(trimmed.to_owned());
        }

        Ok(TestCluster {
            _root: root,
            data_dir,
            username: self.username.clone(),
            database: self.database.clone(),
            port: self.port,
            executed_statements,
        })
    }
}

fn validate_identifier(candidate: &str) -> Result<(), String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return Err(trimmed.to_owned());
    }

    let mut chars = trimmed.chars();
    match chars.next() {
        Some(first) if is_identifier_lead(first) => {}
        _ => return Err(trimmed.to_owned()),
    }

    if chars.all(is_identifier_continue) {
        Ok(())
    } else {
        Err(trimmed.to_owned())
    }
}

const fn is_identifier_lead(value: char) -> bool {
    value.is_ascii_alphabetic()
}

const fn is_identifier_continue(value: char) -> bool {
    value.is_ascii_alphanumeric() || value == '_'
}

const fn validate_port(port: u16) -> Result<(), ClusterError> {
    match port {
        1_024..=65_535 => Ok(()),
        _ => Err(ClusterError::InvalidPort { provided: port }),
    }
}

fn guard_statement(statement: &str, allow_destructive: bool) -> Result<(), ClusterError> {
    if allow_destructive {
        return Ok(());
    }

    let lowered = statement.to_ascii_lowercase();
    if lowered.contains("drop database") || lowered.contains("drop schema") {
        return Err(ClusterError::UnsafeBootstrapStatement {
            statement: statement.to_owned(),
        });
    }

    Ok(())
}

/// Ready-to-use `rstest` fixture that yields a [`TestCluster`] configured with
/// the default builder settings.
#[fixture]
#[must_use]
pub fn test_cluster() -> TestCluster {
    TestCluster::builder()
        .build()
        .unwrap_or_else(|error| panic!("default cluster configuration failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_database() {
        let mut builder = TestCluster::builder();
        builder.database("   ");
        let error = builder.build().expect_err("builder should fail");
        assert!(matches!(error, ClusterError::InvalidDatabaseName { .. }));
    }

    #[test]
    fn rejects_reserved_port() {
        let mut builder = TestCluster::builder();
        builder.port(42);
        let error = builder.build().expect_err("builder should fail");
        assert_eq!(error, ClusterError::InvalidPort { provided: 42 });
    }

    #[test]
    fn rejects_destructive_bootstrap() {
        let mut builder = TestCluster::builder();
        builder.bootstrap_statement("DROP DATABASE demo");
        let error = builder.build().expect_err("bootstrap should fail");
        assert!(matches!(
            error,
            ClusterError::UnsafeBootstrapStatement { .. }
        ));
    }

    #[test]
    fn records_bootstrap_statements() {
        let mut builder = TestCluster::builder();
        builder.bootstrap_statement("CREATE TABLE demo(id INT)");
        let cluster = builder.build().expect("cluster should build");
        assert_eq!(cluster.executed_statements(), ["CREATE TABLE demo(id INT)"]);
    }
}
