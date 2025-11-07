# Ortho Config User's Guide

This guide explains how Whitaker keeps integration-test configuration
orthogonal ("ortho config") so fixtures can be composed without leaking state
across scenarios.

1. **Prefer builders over global state.** `TestCluster::builder()` exposes
   setters for every tunable field. Modify the builder inside fixtures or BDD
   steps rather than mutating environment variables so changes remain local to
   each test.
2. **Layer overrides in a predictable order.** When a scenario needs bespoke
   behaviour, start from the defaults supplied by the shared fixture, apply the
   minimum number of overrides (for example, `builder.database("demo")`), and
   only then call `build()`. Avoid post-construction mutation so assertions can
   treat the returned `TestCluster` as immutable state.
3. **Use `rstest` fixtures for dependency injection.** The exported
   `test_cluster` fixture keeps cluster setup orthogonal to the assertions. BDD
   steps that require additional configuration (for example, destructive
   bootstrap statements) should compose their own fixtures on top of the shared
   one rather than re-implementing the defaults.
4. **Document every assumption.** When a test relies on optional behaviour,
   note it in the relevant Markdown guide (for example, `docs/users-guide.md`)
   so future contributors understand the expected layering order.

Combined with the RAII semantics of `TempDir`, this approach keeps
configuration changes scoped, comprehensible, and easy to reason about under
parallel test execution.
