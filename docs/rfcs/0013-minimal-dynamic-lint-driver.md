# RFC 0013: Minimal dynamic lint driver after CLI completion

## Preamble

- **RFC number:** 0013
- **Status:** Proposed
- **Created:** 2026-10-29
- **Scope:** The compiler execution backend after the unified Whitaker
  command-line interface (CLI) is complete.
- **Roadmap:** [Phase 3 driver
  supplement](../roadmap-phase-3-minimal-dynamic-driver.md).

## Summary

Keep `whitaker-dylint` as the near-term, upstream-derived compatibility fork.
Use it to deliver the binary-only driver contract tracked by
[Whitaker #496](https://github.com/leynos/whitaker/issues/496), with
`whitaker-installer` consuming verified prebuilt binaries from a pinned full
source commit. Do not make that repair depend on a new execution architecture.

After Whitaker has its full core CLI, move Cargo orchestration into that CLI
and reduce the deployed fork to a minimal, separately built dynamic lint
driver. The target execution path is `whitaker check` to Cargo to a prebuilt
compiler driver to verified lint libraries. It does not invoke `cargo-dylint`
or `dylint::run`, and it cannot build a missing driver.

Initially retain the proven compiler-facing implementation derived from
`dylint_driver`, the lint registration and configuration support from
`dylint_linting`, and the required support dependencies. Remove general-purpose
Dylint frontend functionality only after the native path passes the acceptance
corpus. Keep producer and test dependencies separate from the deployed runtime.
This proposal replaces Dylint orchestration, not every Dylint-derived component
at once, and preserves dynamically loaded lint libraries.

## 1. Problem and current state

The inspected Whitaker baseline is commit
[`00cf1a19`](https://github.com/leynos/whitaker/commit/00cf1a19256e4cd4a124f5d638f90e3b83100ae4).
[ADR 009](../adr-009-verified-clippy-snapshot-for-dylint-driver-bootstrap.md)
records the accepted Dylint 6.3.0 bootstrap: verified Clippy history,
identity-keyed driver storage, concurrent-run coordination, and an installer
`run` dispatcher. This is the recovery architecture, not evidence that
binary-only driver delivery or the native CLI has already shipped.

The [CLI design](../whitaker-cli-design.md) preserves dynamic lint libraries
but still delegates checking to `cargo dylint`. The foundation work in
[PR #356](https://github.com/leynos/whitaker/pull/356) does not, by itself,
remove that backend. The root executable, complete CLI, and native compiler
runner are distinct milestones.

Upstream Dylint 6.3.0 separates these responsibilities:

- The `cargo-dylint` executable parses frontend arguments.
- The `dylint` orchestration crate discovers libraries, prepares a driver,
  and launches Cargo.
- `dylint::driver_builder::get` builds a driver when its cached candidate is
  absent, outdated, or cannot be inspected. Its build function also introduces
  the absolute compiler-sysroot runtime search path.[^1]
- `dylint_driver` registers compiler callbacks and loads dynamic lint
  libraries. It preserves library lifetimes, checks the Dylint interface,
  registers matching Clippy symbols, and tracks linting inputs.[^2]

Calling `dylint::run` from a native CLI would retain the driver-build fallback.
A preflight check followed by that call would not eliminate the fallback
boundary. The proposed runner instead avoids that orchestration crate in the
consumer execution path.

The near-term fork is an independent prerequisite, not an implementation in
this RFC. Its pinned-SHA distribution and refusal guard support #496 while the
CLI develops. Neither that bridge nor the eventual native runner depends on
an upstream merge or release accepting the downstream changes.

## 2. Goals, non-goals, and ordering

### 2.1. Goals

- Keep one public Whitaker CLI and a private, compiler-specific executable.
- Preserve dynamic loading, named diagnostics, lint selection, configuration,
  localization, Cargo scope, and supported output and exit-status contracts.
- Make consumer driver provisioning binary-only and explicitly verifiable.
- Keep ordinary CLI code independent of `rustc_private` and its nightly pin.
- Reduce fork ownership without discarding compiler compatibility safeguards.
- Preserve the rolling lint suite while recording an exact artefact generation
  for each invocation. Driver and suite publication need not occur together.

### 2.2. Non-goals

- Replacing the emergency bootstrap or completing #496 in this documentation
  change.
- Beginning runtime replacement before the full core CLI is delivered.
- Writing a compiler, inventing a stable cross-nightly Rust plugin ABI, or
  statically linking the suite into each driver release.
- Supporting arbitrary Git repositories, registry packages, or unverified
  library paths as a new plugin marketplace.
- Removing `dylint_testing`, `dylint-link`, or producer dependencies merely
  because consumers no longer need their executables.
- Relaxing lint scope, warning enforcement, compiler admission, or the rewrite
  sandbox requirements in RFCs 0007 to 0009.

### 2.3. Readiness gate

For this proposal, a full core CLI means delivery and qualification of roadmap
sections 3.5 to 3.9: native `check`, `install`, `ls`, and `doctor`; rule
selection; OrthoConfig merging; bundle state; public release packaging; and the
existing compatibility migration. A root binary alone is insufficient.
Unrelated future rewrite features are not prerequisites for this migration.

Runtime implementation starts only after that gate, the verified outcomes of
[Whitaker #526](https://github.com/leynos/whitaker/issues/526) and
[shared-actions #650](https://github.com/leynos/shared-actions/issues/650), and
the published, qualified fork-based driver delivery required by #496. Closing
an issue without the associated release and consumer evidence does not satisfy
the gate. Design review and dependency inventory may occur earlier, but must
not displace the recovery or CLI work.

## 3. Proposed architecture and ownership

The following flow separates product policy, build orchestration, and
compiler-private execution. The outer compiler wrapper remains in place.

```plaintext
whitaker check
  -> merge configuration and resolve rules
  -> select and verify installed driver and lint generations
  -> construct a typed execution plan
  -> cargo check
       -> existing RUSTC_WRAPPER, when configured
       -> private Whitaker compiler driver
       -> matching rustc_driver + verified dynamic lint libraries
```

*Figure 1: Consumer path without the Dylint frontend or driver builder.*

### 3.1. CLI and installer responsibilities

The CLI owns configuration, rule selection, workspace and target selection,
provisioning policy, Cargo arguments, process lifecycle, and user-facing errors.
The installer owns download authentication, extraction, manifest validation,
atomic generation publication, and installed-state reconciliation. Reuse the
existing installer and CLI boundaries rather than creating a second installer.

Introduce a compiler-independent execution-plan type at the existing internal
boundary. It describes the selected compiler identity and sysroot, host triple,
driver identity and path, ordered library identities and paths, effective lint
configuration, rule-selection result, Cargo scope and arguments, wrapper
composition, and an execution fingerprint. These are proposed responsibilities,
not a new public API or a prescribed crate layout.

Only the validated execution adapter may turn that plan into a child process.
Configuration and domain libraries must not print, terminate the process,
download artefacts, or expose compiler-private types.

### 3.2. Minimal driver responsibilities

Build a small private executable in `whitaker-dylint` against its pinned
compiler-facing core. Upstream's generated executable already demonstrates the
entry-point boundary through `dylint_driver::dylint_driver`; retaining that core
is the initial approach, not a clean-room rewrite.[^1]

The retained core must continue to handle:

- Cargo compiler-wrapper arguments and compiler queries;
- compiler callbacks and the approved dynamic-library registration protocol;
- Dylint interface checks, missing symbols, loader failures, and safe library
  lifetimes, including thread-local teardown;
- matching Clippy symbols and their ordering;
- `dylint_lib` configuration values used by existing lint and support macros;
- dependency-linting policy, diagnostics, and compiler outcome propagation;
- dependency tracking and compiler incremental-cache invalidation.

The driver receives explicit verified libraries and configuration. It validates
required execution inputs itself before compiler entry, rather than trusting
only a CLI preflight. Metadata and compiler-query modes have explicit separate
contracts. Missing execution context must not activate discovery or an empty
lint session. It does not search repositories, resolve dependencies, install
tools, update itself, or compile another driver. Network-capable acquisition
and source-build fallback
must not remain reachable from its consumer entry points.

### 3.3. Component retention and removal

Remove `cargo-dylint` and general-purpose `dylint` orchestration from the native
consumer path. Initially retain `dylint_driver`, `dylint_linting`, and the
support code their validated dependency graph requires. Do not assume that a
crate can be deleted because its name resembles frontend infrastructure.

Keep `dylint-link` in producer tooling while library naming needs it. Keep
`dylint_testing` and Whitaker's wrapper-parsing patch in development and tests;
its transitive orchestration dependencies do not make them consumer runtime
requirements. Inventory these graphs independently before removing packages,
features, tests, workflows, or documentation from the fork.

## 4. Execution and compatibility contracts

### 4.1. Binary identity and trust

Reuse and extend #496's manifest contract rather than inventing an unrelated
asset format. Bind each driver artefact to the full fork source commit,
upstream baseline, driver protocol/schema version, immutable Rust compiler
commit, host architecture and triple, supported platform application binary
interface (ABI), archive digest, installed executable digest, and required
compiler runtime libraries. Record the selected toolchain alias separately;
an alias alone does not establish compiler identity.

Authenticate the manifest against the configured producer identity and pinned
source revision, using a trusted digest or verified build attestation under the
release policy. A source SHA does not authenticate downloaded executable bytes,
and an untrusted neighbouring checksum file does not establish provenance.
Never invent release hashes before publication. Record the lockfile and
compiler inputs used by the producer so builds remain auditable.

Verify the driver, compiler, and lint-library compatibility tuple before
loading a library. The Dylint interface-version check is necessary but does not
prove `rustc_private` ABI compatibility. The driver host is distinct from a
Cargo `--target` being checked; cross-compilation must select a host-native
driver, not an executable for the target architecture.

Installation stages immutable generations, verifies them, and publishes them
atomically. An invocation pins one complete compatible generation and holds the
necessary lease until its children finish. Concurrent installation, cleanup,
and checking must not mix driver and suite generations or delete live files.
Locks protect cooperating processes, not a hostile process with the same
filesystem permissions. Loading a lint library executes trusted native code;
this is not a sandbox for untrusted plugins.

### 4.2. Provisioning and failure boundaries

Preserve the core CLI's lazy-repair, `--no-install`, and `--offline` contracts.
Lazy repair may acquire a verified published driver only where policy permits;
`--no-install` must not acquire it, and offline mode must not contact a remote.
Missing, corrupt, mismatched, unsupported, or uninspectable driver assets must
produce actionable operational errors. None may trigger driver compilation,
plain-rustc success, a reduced lint set, or automatic use of the legacy backend.

The existing explicit source-build option for lint bundles must not become an
implicit driver-build permission. Driver source builds belong to the controlled
producer/developer workflow, outside the consumer checking path. Target-project
compilation and its normal build scripts remain expected: binary-only driver
provisioning is not a promise that `cargo check` never invokes a compiler.

### 4.3. Cargo, wrappers, and scope

Launch the selected Cargo and compiler without shell-string interpolation.
Preserve argument boundaries, working directory, package/workspace selection,
feature flags, target selection, and trailing Cargo arguments. Retain the
resolved target-directory policy and compiler admission/cache wrapper rather
than introducing per-invocation cold temporary targets.

Cargo nests `RUSTC_WRAPPER` outside `RUSTC_WORKSPACE_WRAPPER`.[^3] Use that
contract for the private driver, preserving the existing outer wrapper. Resolve
wrapper settings from Cargo configuration as well as environment variables.
An existing workspace wrapper requires a tested composition strategy or an
explicit conflict error; silently overwriting it is not acceptable. Detect
recursive invocation and preserve query-only compiler probes.

Do not turn library selection into package selection: selecting every installed
lint library does not imply checking every Cargo workspace member. Prove scope
with violations in non-default members, not just a successful exit code.
Preserve effective Rust flags and encoded-flag precedence; do not erase warning
policy to make the new runner pass.

### 4.4. Configuration, selection, and library loading

The CLI remains the authority for CLI-over-environment-over-file configuration,
canonical rule names/codes, experimental rules, and compatibility notices.
Pass a resolved snapshot to the driver; it must not independently rediscover a
conflicting configuration or accept ambient library-path overrides.

The first adapter may use the retained core's `DYLINT_LIBS` JSON path list,
`DYLINT_TOML` configuration payload, and associated fingerprint inputs as a
private compatibility transport.[^2][^4] These names are not a renewed public
configuration API. Set them only in the child environment, remove conflicting
inherited backend-owned values, and retain unrelated build-admission settings.
Bound and validate payloads, including Windows environment limits; select a
versioned file transport before release if the supported payload cannot fit.

Reject malformed or unexpectedly empty library selections before compilation.
The upstream path parser can otherwise default to an empty list.[^2] An explicit
valid zero-rule selection may follow the CLI's documented no-op contract, but
must not disguise missing libraries or configuration failures as a clean run.

Keep the existing registration symbols, lint namespaces, attribute behaviour,
and library filename/toolchain conventions during the first cutover. Changing
them requires its own compatibility decision, not an opportunistic cleanup.

### 4.5. Freshness and incremental correctness

Treat Cargo freshness and rustc incremental reuse as separate obligations.
The execution fingerprint covers driver identity, compiler identity, ordered
library identities and bytes, effective configuration, rule selection, locale
where diagnostic output changes, dependency scope, and relevant lint flags.

Retain the upstream driver's file/environment dependency tracking and
untracked-state hashing while establishing equivalent behaviour.[^2] Ensure a
changed input makes Cargo invoke the driver and also prevents stale compiler
incremental results. Updating a hash inside a driver that Cargo never starts
cannot satisfy the first obligation.

Prove changes at the same installed path, including same-size content changes
with restored timestamps. A stable identity namespace or declared tracked
inputs must invalidate correctly without deleting the whole target directory.
Unchanged warm runs must remain reusable. Selecting a fresh temporary target
for every check is not an acceptable substitute for an invalidation contract.

### 4.6. Diagnostics and process lifecycle

Preserve lint levels, named diagnostics, spans, suggestions, stream selection,
Cargo JSON output, supported Static Analysis Results Interchange Format (SARIF)
output, locale behaviour, and lint-failure exit semantics. Use the established
CLI operational-error taxonomy for
provisioning, compatibility, and execution failures; do not allocate new public
exit codes without updating that contract.

Keep product status output out of machine-readable compiler stdout. Propagate
cancellation and signals to the process tree, retain bounded diagnostic context
for `doctor`, and release generation leases after child termination. `ls` and
`doctor` must report selected driver provenance and compatibility without
building a driver. Do not rely exclusively on stale manifest metadata to claim
that a runtime can start or that an installed lint catalogue matches its bytes.

## 5. Producer builds and relocation

Use the pinned fork revision and recorded compiler/Clippy inputs to build and
publish the private executable. Clippy history and symbol generation remain
producer concerns. Retain ADR 009's verified snapshot wherever producer or
legacy/test paths still require it; do not remove the snapshot mechanism
merely because the new consumer path stops reading it.

Qualify every supported host/compiler combination with a producer-independent
installation root and compiler sysroot. Make the producer's home directory and
original sysroot unavailable. Exercise the real lint suite, not only
`--version`.
Inspect executable and dynamic-library dependencies, the GNU C Library (glibc)
floor where applicable, and platform loader behaviour. A checksum or matching
nightly label is not relocation evidence.

The executable build and launch adapter must establish runtime lookup against
the verified destination toolchain without relying on the producer's absolute
runtime search path (RPATH) or an ambient broad library search path. The exact
Linux, macOS, and Windows strategy remains a gated implementation decision.
Keep unsupported combinations explicitly unsupported; do not infer driver
support from the existing suite archive matrix.

## 6. Acceptance and evidence

Build a differential corpus against the pinned, qualified fork backend using
the same compiler, suite, effective configuration, and Cargo scope. Normalize
only documented incidental values such as temporary paths. Do not normalize
away diagnostic names, source locations, severity, suggestions, selected rules,
package coverage, or exit status.

Release evidence must cover the following scenarios:

- Cold and warm runs of the actual suite, including deliberate named
  `no_expect_outside_tests` and `no_std_fs_operations` violations and clean
  controls. Include each managed bundle and conditional suppression macros.
- Workspaces with default and non-default members; explicit packages; tests,
  examples, and feature combinations; supported cross-target checking; and
  dependency-linting policy. Record which crates actually reached the driver.
- Configuration precedence, canonical and legacy migration behaviour, rule
  families, experimental rules, locale changes, invalid payloads, explicit
  zero-rule selection, and unexpected empty/malformed library lists.
- Driver, compiler, suite, and config changes with an otherwise warm target,
  including same-path/same-timestamp library replacement, and unchanged warm
  reuse under the normal compiler cache/admission chain.
- Missing, stale, corrupt, ABI-mismatched, wrong-host, and uninspectable assets;
  missing registration symbols; interrupted installation; concurrent checks,
  upgrade, cleanup, and rollback; and wrapper conflicts or recursion.
- Relocation on each published host, with producer paths unavailable; paths
  containing spaces and supported non-ASCII characters; compiler queries;
  machine-readable output; cancellation; and non-zero compiler exits.
- A fresh controlled consumer with target dependencies prefetched, network
  denied for the test, no `cargo-dylint` executable, no consumer Clippy
  snapshot,
  and no available driver-build fallback. Trace process execution and assert
  that no driver construction or tool installation ran. Do not disable Cargo
  or rustc themselves: the target fixture still needs them.

Use unit and property tests for manifest matching, configuration normalization,
argument/environment construction, and fingerprint invariants. Use behavioural
and end-to-end tests for actual process and loader contracts. Exercise negative
controls that deliberately remove a library, change a rule/configuration, omit
workspace scope, or bypass the refusal boundary; the corpus must detect each
regression. Retain producer, test-harness, and wrapper-parsing regression
suites.

Record cold/warm time, build-slot use, download size, and installed footprint
against the qualified baseline. These measurements inform pruning; no specific
speed-up or size reduction is assumed. Correctness and scope parity gate
promotion regardless of performance.

## 7. Compatibility, rollout, and document precedence

The [Phase 3 supplement](../roadmap-phase-3-minimal-dynamic-driver.md) owns the
proposed task sequence. It adds work after the current CLI milestones without
renumbering them or duplicating #496's producer/provisioning implementation.

During qualification, select the native backend explicitly in the test or
pilot harness. Keep the previous qualified fork-based backend available as an
operator-selected rollback, with its binary-only policy intact. A native
failure must never silently retry through the legacy backend. Switch the
normal consumer path only after the documented host matrix and representative
consumer gates pass.

Retire frontend distribution and then prune the fork in separate changes, each
backed by a dependency inventory and regression evidence. Remove consumer
Clippy snapshot downloads only when neither its supported execution nor repair
path needs them. Producer and test retention can continue independently.

This RFC is proposed and changes no shipped behaviour or accepted ADR today.
On acceptance and implementation, amend the CLI design's Dylint delegation and
ADR 009's consumer first-use-build decision with a new decision record. Retain
the historical recovery rationale. Preserve unrelated CLI semantics and the
rewrite process-tree isolation requirements. Update user/developer/publishing
guides, `ls`/`doctor`, and shared-action documentation at actual rollout, not as
though this proposal already implements them.

## 8. Alternatives considered

### 8.1. Keep the full fork indefinitely

This is a valid compatibility baseline, but leaves general-purpose discovery,
frontend policy, and driver acquisition alongside a CLI that already owns those
responsibilities. Retain it for recovery and rollback, not as the intended
minimal consumer architecture.

### 8.2. Call `dylint::run` from the native CLI

This removes a process boundary but retains orchestration and driver-build
fallback. It does not establish the required binary-only execution boundary.

### 8.3. Rewrite the loader and compiler adapter immediately

This would take ownership of compiler-version branches, symbol registration,
library lifetimes, and incremental correctness before proving a benefit. Keep
the existing core first and remove code only with evidence.

### 8.4. Link the suite statically into the driver

This removes dynamic loading but ties driver production more closely to every
suite update. It conflicts with the required rolling, dynamically loaded suite
and is outside this proposal.

### 8.5. Depend directly on upstream compiler-facing crates

This is a desirable later simplification if unmodified upstream crates meet the
qualified contracts. Evaluate it after native parity; it is not a prerequisite
for using or reducing `whitaker-dylint`, and it does not justify deleting a
necessary downstream compatibility fix.

## 9. Open implementation decisions

- Select the private executable name, package layout, and internal plan/schema
  boundary after inspecting the delivered CLI; avoid duplicate runner modules.
- Qualify a destination-toolchain loader strategy for every published host.
- Select the exact Cargo-freshness mechanism and bounded configuration transport
  with the adversarial warm-cache corpus, rather than assuming compatibility.
- Confirm which fork support features remain in runtime, producer, and test
  graphs, and whether any can return to unmodified upstream dependencies.

## Recommendation

Adopt the staged minimal dynamic driver direction after full core CLI delivery.
Use the pinned, prebuilt fork to finish the immediate binary-only work, then
move orchestration into Whitaker, qualify the retained compiler core, and prune
only code that the verified native path no longer needs.

[^1]: Dylint 6.3.0 [driver builder and executable template](https://github.com/trailofbits/dylint/blob/v6.3.0/dylint/src/driver_builder.rs).
[^2]: Dylint 6.3.0 [compiler driver](https://github.com/trailofbits/dylint/blob/v6.3.0/driver/src/lib.rs)
    and [untracked-state handling](https://github.com/trailofbits/dylint/blob/v6.3.0/driver/src/untracked_state.rs).
[^3]: Cargo's [compiler-wrapper environment contract](https://doc.rust-lang.org/cargo/reference/environment-variables.html).
[^4]: Dylint 6.3.0 [lint configuration support](https://github.com/trailofbits/dylint/blob/v6.3.0/utils/linting/src/lib.rs).
