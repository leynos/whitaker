# Zero-Config RAII Postgres Test Fixture

This document records the design decisions behind the
`whitaker::testing::cluster` module, which exposes an ergonomic, disposable
Postgres-style cluster for integration tests.

## Objectives

- Provide a "works by default" fixture (`test_cluster`) so `rstest` suites can
  exercise integration behaviour without hand-written setup.
- Keep the implementation hermetic: tests should not require Docker or a real
  Postgres binary, yet still surface realistic ergonomics such as connection
  URIs and bootstrap scripts.
- Enforce safe defaults via RAII so temporary directories are cleaned
  automatically and destructive SQL statements are blocked unless explicitly
  enabled.

## Key Decisions

1. **Simulated cluster backed by `tempfile::TempDir`:** The fixture allocates a
   UTF-8 data directory inside a `TempDir`. Dropping the `TestCluster` removes
   the directory automatically, satisfying the RAII requirement and keeping
   tests isolated across processes.
2. **Validation mirrors common Postgres rules:** Usernames and database names
   must start with an ASCII alphabetic character and may only contain
   alphanumerics or underscores. Ports must live in the 1024–65535 range so
   tests never conflict with privileged sockets.
3. **Destructive bootstrap statements are denied by default:** `DROP DATABASE`
   and `DROP SCHEMA` statements raise `ClusterError::UnsafeBootstrapStatement`.
   Test authors can opt in via `allow_destructive_bootstrap(true)` when they
   intentionally exercise failure flows.
4. **`rstest` fixture baked into the library:** Exporting the `#[fixture]
   fn test_cluster() -> TestCluster` helper lets downstream integration tests
   opt in by naming the fixture in their parameter list without declaring any
   boilerplate. The fixture panics if the builder fails, keeping failures loud
   and early.
5. **Statement recording instead of a live database:** Rather than launching a
   real database, the builder records all bootstrap statements. Tests can
   assert that specific migrations ran while enjoying deterministic execution
   in CI.

## Testing Strategy

- **Unit tests** cover invalid identifiers, unsafe statements, and successful
  bootstrap logging inside `src/testing/cluster.rs`.
- **Behaviour-driven tests** (`tests/test_cluster.rs` and
  `tests/features/test_cluster.feature`) exercise the exported fixture plus the
  happy and unhappy builder paths, ensuring `rstest-bdd` remains the canonical
  way to describe integration expectations.
- **Doctests and README excerpts** show the intended usage so future
  contributors reach for the fixture before re-implementing ad hoc setup.

## Future Enhancements

- Allow callers to specify a fake password so connection URIs more closely
  resemble production deployments.
- Store structured bootstrap metadata (for example, tags or execution timing)
  so behaviour tests can assert on migration ordering without parsing strings.
- Provide serial-test guards for future environment mutations (for example,
  injecting `PGHOST`) so the fixture can evolve toward launching a real
  database when needed.
