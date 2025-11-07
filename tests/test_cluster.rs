//! Behaviour-driven tests for the zero-config `TestCluster` fixture.

use std::{cell::RefCell, str::FromStr};

use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker::testing::cluster::{ClusterError, TestCluster, TestClusterBuilder, test_cluster};

#[derive(Debug)]
struct StepString(String);

impl FromStr for StepString {
    type Err = core::convert::Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let trimmed = input.trim();
        Ok(Self(trimmed.trim_matches('"').to_owned()))
    }
}

impl StepString {
    fn into_inner(self) -> String {
        self.0
    }
}

fn normalised_statement(value: &str) -> String {
    value
        .trim()
        .trim_matches(|candidate| matches!(candidate, '"' | '\''))
        .to_owned()
}

#[derive(Default)]
struct ClusterWorld {
    builder: RefCell<TestClusterBuilder>,
    result: RefCell<Option<Result<TestCluster, ClusterError>>>,
}

#[fixture]
fn cluster_world() -> ClusterWorld {
    ClusterWorld::default()
}

#[given("a fresh cluster builder")]
fn reset_builder(cluster_world: &ClusterWorld) {
    *cluster_world.builder.borrow_mut() = TestCluster::builder();
    cluster_world.result.borrow_mut().take();
}

#[given("the database name is {name}")]
fn override_database(cluster_world: &ClusterWorld, name: String) {
    cluster_world.builder.borrow_mut().database(name);
}

#[given("the username is {name}")]
fn override_username(cluster_world: &ClusterWorld, name: String) {
    cluster_world.builder.borrow_mut().username(name);
}

#[given("the cluster port is {port}")]
fn override_port(cluster_world: &ClusterWorld, port: u16) {
    cluster_world.builder.borrow_mut().port(port);
}

#[given("a bootstrap statement {statement} is queued")]
fn queue_statement(cluster_world: &ClusterWorld, statement: String) {
    cluster_world
        .builder
        .borrow_mut()
        .bootstrap_statement(statement);
}

#[given("destructive bootstrap statements are allowed")]
fn allow_destructive(cluster_world: &ClusterWorld) {
    cluster_world
        .builder
        .borrow_mut()
        .allow_destructive_bootstrap(true);
}

#[when("the cluster is built")]
fn build_cluster(cluster_world: &ClusterWorld) {
    let builder = cluster_world.builder.borrow().clone();
    let outcome = builder.build();
    cluster_world.result.borrow_mut().replace(outcome);
}

#[then("building succeeds with database {name}")]
fn assert_success(cluster_world: &ClusterWorld, name: StepString) {
    let borrow = cluster_world.result.borrow();
    let Some(outcome) = borrow.as_ref() else {
        panic!("cluster should be built");
    };

    let expected = name.into_inner();

    match outcome {
        Ok(cluster) => assert_eq!(cluster.database(), expected.as_str()),
        Err(error) => panic!("expected cluster success, found {error}"),
    }
}

#[then("the applied statements include {statement}")]
fn assert_statements(cluster_world: &ClusterWorld, statement: StepString) {
    let borrow = cluster_world.result.borrow();
    let Some(outcome) = borrow.as_ref() else {
        panic!("cluster should be built");
    };

    let cluster = match outcome {
        Ok(cluster) => cluster,
        Err(error) => panic!("expected cluster success, found {error}"),
    };
    let value = statement.into_inner();
    let expected = normalised_statement(value.as_str());
    assert!(
        cluster
            .executed_statements()
            .iter()
            .any(|candidate| normalised_statement(candidate) == expected),
        "expected bootstrap statements to include {expected}"
    );
}

#[then("building fails with an invalid database name error")]
fn assert_invalid_database(cluster_world: &ClusterWorld) {
    let borrow = cluster_world.result.borrow();
    let Some(result) = borrow.as_ref() else {
        panic!("cluster should be built");
    };

    match result {
        Err(ClusterError::InvalidDatabaseName { .. }) => {}
        Err(other) => panic!("expected invalid database error, found {other}"),
        Ok(cluster) => panic!("expected failure, found success for {cluster:?}"),
    }
}

#[then("building fails with an invalid port error")]
fn assert_invalid_port(cluster_world: &ClusterWorld) {
    let borrow = cluster_world.result.borrow();
    let Some(result) = borrow.as_ref() else {
        panic!("cluster should be built");
    };

    match result {
        Err(ClusterError::InvalidPort { .. }) => {}
        Err(other) => panic!("expected invalid port error, found {other}"),
        Ok(cluster) => panic!("expected failure, found success for {cluster:?}"),
    }
}

#[then("building fails with an unsafe statement error")]
fn assert_unsafe_statement(cluster_world: &ClusterWorld) {
    let borrow = cluster_world.result.borrow();
    let Some(result) = borrow.as_ref() else {
        panic!("cluster should be built");
    };

    match result {
        Err(ClusterError::UnsafeBootstrapStatement { .. }) => {}
        Err(other) => panic!("expected unsafe statement error, found {other}"),
        Ok(cluster) => panic!("expected failure, found success for {cluster:?}"),
    }
}

#[scenario(path = "tests/features/test_cluster.feature", index = 0)]
fn scenario_builds_with_defaults(cluster_world: ClusterWorld) {
    let _ = cluster_world;
}

#[scenario(path = "tests/features/test_cluster.feature", index = 1)]
fn scenario_rejects_invalid_database(cluster_world: ClusterWorld) {
    let _ = cluster_world;
}

#[scenario(path = "tests/features/test_cluster.feature", index = 2)]
fn scenario_rejects_reserved_port(cluster_world: ClusterWorld) {
    let _ = cluster_world;
}

#[scenario(path = "tests/features/test_cluster.feature", index = 3)]
fn scenario_records_bootstrap(cluster_world: ClusterWorld) {
    let _ = cluster_world;
}

#[scenario(path = "tests/features/test_cluster.feature", index = 4)]
fn scenario_blocks_destructive_bootstrap(cluster_world: ClusterWorld) {
    let _ = cluster_world;
}

#[rstest]
fn fixture_requires_no_setup(test_cluster: TestCluster) {
    assert_eq!(test_cluster.username(), "postgres");
    assert!(
        test_cluster
            .connection_uri()
            .contains(test_cluster.database())
    );
}
