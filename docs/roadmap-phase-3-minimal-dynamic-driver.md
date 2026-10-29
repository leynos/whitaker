# Phase 3 minimal dynamic driver roadmap supplement

This supplement proposes the post-CLI extension to
[roadmap Phase 3](roadmap.md#3-aggregated-packaging-installer-and-unified-cli).
[RFC 0013](rfcs/0013-minimal-dynamic-lint-driver.md) defines its contracts and
acceptance criteria. Sections 3.10 to 3.14 extend the existing numbering without
renumbering or replacing tasks 3.1 to 3.9. All tasks remain proposed and
unchecked.

The existing CLI milestones deliver the complete core product first. This
supplement then removes the Dylint frontend from the consumer execution path
while retaining and qualifying the minimum compiler-facing dynamic loader.
It does not replace the emergency repair or duplicate #496's implementation.

## Dependency and ownership boundaries

- [Whitaker #526](https://github.com/leynos/whitaker/issues/526) and
  [shared-actions #650](https://github.com/leynos/shared-actions/issues/650)
  own reproducible bootstrap and its consumer integration. Require their
  published and verified outcomes, not issue closure alone.
- [Whitaker #496](https://github.com/leynos/whitaker/issues/496) owns the
  near-term prebuilt compiler-driver assets and provisioning. The planned
  `whitaker-dylint` fork supplies the refusal change and pinned source identity;
  the installer must also verify binary identity and compatibility. Do not
  substitute a future native runner for that immediate deliverable.
- [PR #356](https://github.com/leynos/whitaker/pull/356) and roadmap sections
  3.5 to 3.9 own core CLI delivery, configuration, installation, diagnostics,
  and compatibility migration. Native runtime replacement starts afterwards.
- Producer and test tooling may retain Dylint dependencies after consumer
  runtime reduction. Removal requires separate dependency and behaviour proof.

## 3. Aggregated packaging, installer, and unified CLI extension

### 3.10. Post-CLI readiness and execution inventory

- [ ] 3.10.1. Record the runtime-replacement readiness gate. Require verified
  delivery of roadmap sections 3.5 to 3.9, including published `check`,
  `install`, `ls`, and `doctor`, rule/configuration semantics, and compatibility
  migration, plus the qualified #496 fork release and #526/#650 outcomes.
  Requires 3.5.3, 3.8.5, and 3.9.3.
  - Success: the record names immutable release evidence and consumer runs; a
    root binary or configuration-only PR cannot satisfy the gate. Unrelated
    future rewrite features do not block it. See RFC 0013 section 2.3.
- [ ] 3.10.2. Inventory consumer, producer, and test dependency graphs and
  capture the pinned legacy backend's execution contract. Identify existing CLI
  boundaries before proposing new runner modules. Produce the implementation
  ExecPlan and resolve package/schema ownership. Requires 3.10.1.
  - Success: the inventory names retained `dylint_driver`, `dylint_linting`,
    support features, linker tooling, and the patched test harness; records
    Cargo arguments, environment, diagnostics, and supported host/compiler
    pairs; and identifies the driver-build call path that consumers will lose.

### 3.11. Minimal dynamic driver and verified distribution

- [ ] 3.11.1. Extend the #496 manifest and compatibility contract for the
  private driver without creating a competing artefact schema. Bind fork SHA,
  upstream baseline, compiler commit, host/ABI, protocol/schema, archive and
  executable digests, and runtime dependencies. Requires 3.10.2 and the
  qualified #496 distribution.
  - Success: authenticity and digest checks reject substituted, wrong-host,
    stale, mismatched, or uninspectable assets before dynamic loading. Source
    identity and binary identity remain distinct. See RFC 0013 section 4.1.
- [ ] 3.11.2. Build the private executable against the pinned compiler-facing
  core in `whitaker-dylint`. Retain dynamic-library registration, library
  lifetimes, matching Clippy symbols, compiler callbacks, conditional lint
  configuration, and dependency tracking. Requires 3.11.1.
  - Success: the actual suite emits named diagnostics without calling
    `cargo-dylint` or `dylint::run`; the runtime has no reachable driver-build,
    dependency-acquisition, or self-update path. See RFC 0013 section 3.2.
- [ ] 3.11.3. Qualify driver and library relocation for each published
  host/compiler pair with different installation and sysroot paths and no
  access to producer paths. Resolve loader search and glibc constraints where
  applicable. Requires 3.11.2.
  - Success: real diagnostics and negative loader cases pass independently on
    every supported host; untested combinations remain unsupported. Do not
    infer support from the suite archive matrix. See RFC 0013 section 5.
- [ ] 3.11.4. Integrate verified native-driver generations with the delivered
  CLI installer, atomic publication, live-generation leases, and rollback
  records. Preserve lazy repair, offline, and no-install policy while refusing
  all consumer driver source builds. Requires 3.11.3 and 3.7.3.
  - Success: concurrent install/check/cleanup and interrupted publication tests
    cannot mix generations or remove live artefacts; negative asset cases fail
    closed. Reuse the existing installation boundary rather than duplicating it.

### 3.12. Native execution, configuration, and cache correctness

- [ ] 3.12.1. Implement a compiler-independent execution plan and native Cargo
  adapter in the full CLI. Preserve package/workspace/feature/target scope,
  argument boundaries, effective target routing, compiler queries, warning
  policy, and outer compiler admission/cache wrappers. Requires 3.11.4.
  - Success: wrapper nesting and conflicts are explicit and tested, including
    Cargo-configured wrappers, recursion, non-default workspace members, and
    host-native driver selection for cross-target checks. See RFC 0013
    sections 3.1 and 4.3.
- [ ] 3.12.2. Bridge resolved OrthoConfig settings and rule selections to the
  retained lint libraries through a validated child-only transport. Preserve
  locale, experimental selectors, and the completed public migration contract.
  Requires 3.12.1, 3.6.2, and 3.6.4.
  - Success: malformed/empty library lists fail rather than silently disabling
    linting; valid explicit zero-rule requests follow the CLI contract; ambient
    backend variables cannot override managed inputs. Qualify transport size
    limits on Windows. See RFC 0013 section 4.4.
- [ ] 3.12.3. Implement and qualify both Cargo-freshness and rustc-incremental
  invalidation for driver, compiler, library bytes, effective configuration,
  selectors, locale, and lint scope. Requires 3.12.2.
  - Success: same-path/same-timestamp changes alter the observed diagnostics
    without cleaning the whole target; unchanged warm runs remain reusable.
    Changing a driver-local hash alone is insufficient. See RFC 0013
    section 4.5.
- [ ] 3.12.4. Integrate native runtime identity, errors, and lint catalogue
  validation with `ls`, `doctor`, failure records, and process cancellation.
  Preserve machine-readable streams and lint-failure exit semantics. Requires
  3.12.3 and 3.8.5.
  - Success: status commands do not build drivers; operational failures remain
    distinct from lint failures; cancellation terminates children before leases
    release; supported text, JSON, and SARIF contracts remain intact.

### 3.13. Differential qualification and controlled cutover

- [ ] 3.13.1. Execute RFC 0013 section 6's differential corpus against the
  pinned fork baseline and native backend. Add unit/property coverage and
  behavioural/end-to-end negative controls for the new contracts. Requires
  3.12.4.
  - Success: cold/warm diagnostics, rule selection, workspace coverage,
    configuration changes, relocation, wrappers, concurrency, output, and exit
    status match. A prefetched, network-denied consumer works without
    `cargo-dylint` or a Clippy snapshot and proves no driver build ran, while
    still allowing compilation of the target fixture.
- [ ] 3.13.2. Pilot the native backend explicitly on representative consumers
  and every supported host/compiler pair. Record cold/warm timings, build-slot
  use, download size, and footprint without assuming an improvement. Requires
  3.13.1.
  - Success: previously affected consumers, including the relevant Femtologging
    lint lanes, and a non-default-member workspace retain their actual lint
    scope and warning gates. Publish a qualified operator-selected rollback.
- [ ] 3.13.3. Promote the qualified native path and update the CLI design,
  accepted-decision records, user/developer/publishing guidance, and shared
  installation contracts. Requires 3.13.2.
  - Success: no automatic fallback masks native errors. The decision amendment
    explicitly replaces consumer first-use driver construction while preserving
    ADR 009's historical rationale and unaffected policies. Rewrite sandbox
    obligations remain unchanged. See RFC 0013 section 7.

### 3.14. Evidence-led fork reduction

- [ ] 3.14.1. Remove general-purpose frontend/orchestration distribution from
  the native consumer path after the documented rollback/compatibility window.
  Remove consumer snapshot downloads only when no supported consumer path
  needs them. Requires 3.13.3.
  - Success: installed-file and process inventories show no consumer
    `cargo-dylint`/driver-builder dependency; no supported legacy installation
    loses its required artefacts or recovery path.
- [ ] 3.14.2. Prune unused fork packages, features, and workflows in small,
  separately validated changes. Preserve the producer's symbol inputs,
  `dylint-link` where required, `dylint_testing`, and Whitaker's test patch
  until
  replacements have independent parity evidence. Requires 3.14.1.
  - Success: runtime, producer, and test dependency graphs document each
    removal; the acceptance corpus and retained upstream regression tests pass.
- [ ] 3.14.3. Evaluate direct use of unmodified upstream compiler-facing crates
  and record the retained patch/upstream-refresh policy. Requires 3.14.2.
  - Success: maintenance responsibility and compiler-upgrade qualification are
    explicit. Removing the fork is optional and evidence-led, not a prerequisite
    for the minimal dynamic driver or a reason to weaken its contracts.
