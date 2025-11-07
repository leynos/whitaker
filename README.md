# Whitaker

Whitaker ships shared infrastructure for building and testing Dylint crates. In
addition to the lint template and UI harness, it now exposes an ergonomic
`rstest` fixture that provisions a disposable `TestCluster` for integration
tests.

## Zero-config test cluster fixture

Import the shared fixture and list `test_cluster` in your `#[rstest]`
parameters to receive a ready-to-use Postgres-style cluster. No manual setup or
teardown is required; the `TempDir` backing the cluster is deleted
automatically when the fixture drops.

```rust
use rstest::rstest;
use whitaker::testing::cluster::{test_cluster, TestCluster};

#[rstest]
fn verifies_schema(test_cluster: TestCluster) {
    assert!(test_cluster.connection_uri().starts_with("postgresql://"));
    assert_eq!(test_cluster.database(), "whitaker_test");
}
```

For bespoke scenarios, start from `TestCluster::builder()` and override
identifiers, ports, or bootstrap statements before calling `build()`. The
builder validates identifiers, rejects destructive SQL by default, and records
applied statements so behavioural tests can assert on setup without touching a
real database.
