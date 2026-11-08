# Debugging Record: Kani Adjacency Proof Memory Cap

- Generated: 2026-10-26 06:03 UTC
- Issue ID: PR-365-KANI-OOM
- Severity: High
- Falsification sub-agent: `scrutineer` (fallback; `alchemist` unavailable)

## Problem Statement

The adjacency Kani harnesses for no-spurious edges, symmetry, bounds, edge
preservation, and row count exceeded their 8 GiB process-tree memory cap. The
generated Proptest now covers all these properties through the production
builder, alongside sorted-neighbour ordering.

## Context Summary

| Aspect              | Details                                                                                                             |
| ------------------- | ------------------------------------------------------------------------------------------------------------------- |
| First observed      | Candidate tree `91cea89efb69317452ab6230ad231115976e52bc`                                                           |
| Reproduction rate   | One 8 GiB OOM on this tree; prior trees timed out in stable sort                                                    |
| Affected components | `community_kani/adjacency.rs`, `community_kani/shared.rs`, `community.rs`                                           |
| Recent changes      | Adjacency Kani models were removed after repeated cap failures; a generated Proptest covers the production builder. |

### Error Artefacts

```text
Generated 76017 VCC(s), 30643 remaining after simplification
Kani resources: harness=decomposition-all exit_status=143
duration_seconds=338 peak_memory_mib=8192 memory_cap_mib=8192
scope_result=oom-kill
Kani exceeded the process-tree memory cap; proof is inconclusive
```

### Information Gaps

- The precise contribution of symbolic edge cardinality, endpoint choices,
  weights and `Vec` allocation to the SAT memory peak is unknown.
- The timed-out proof's solver trace does not identify which input dimension
  dominates the remaining 30,643 verification conditions.

______________________________________________________________________

## Hypotheses

### H1: The symbolic three-edge domain drives the memory peak

**Claim**: Symbolic edge count, endpoints and weights in the three-node model
drive the first harness beyond the memory cap; the standard sort implementation
is no longer the dominant cost.

**Plausibility**: High — the current proof no longer reaches sort internals,
but still explores up to three symbolic edges with symbolic endpoints and
64-bit weights.

**Prediction**: If this hypothesis holds, lowering only the model's maximum
edge count from three to two will let the focused no-spurious-edge harness
complete under the existing 8 GiB cap. Two edges still exercise nontrivial
adjacency buckets, symmetry and edge preservation.

#### H1 Falsification Plan

| Step | Action                                                                                                                                                        | Expected Negative Result                                                                                                      |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| 1    | Temporarily set `MAX_EDGES` to 2 in `community_kani/shared.rs`, then run `./scripts/run-kani.sh decomposition verify_build_adjacency_no_spurious_edges` once. | The hypothesis is falsified if the focused harness still reaches the 8 GiB cap or times out before producing a proof verdict. |
| 2    | Restore the staged candidate exactly after capturing the experiment result.                                                                                   | A changed tree after restoration invalidates the experiment and must be reported.                                             |

**Tooling**: The repository's `run-kani.sh` and its per-process limited runner;
shared Cargo cache and compiler admission remain enabled.

**Confidence on falsification**: A memory-cap failure with two edges would
strongly falsify edge cardinality as the decisive cause. A pass would support
the hypothesis but would not establish the original three-edge proof.

#### H1 Result

**Verdict**: Falsified with high confidence. The focused two-edge run still
generated 76,017 VCCs, retained 30,643 after simplification, and hit the 8 GiB
cap after 320 seconds. See `/tmp/pr365-alchemist-h1-max-edges-2.out`.

The edge-count bound alone does not reduce the symbolic state. The next
experiment removes symbolic endpoint generation and edge permutations while
retaining every simple three-node topology and arbitrary positive weights.

### H2: Symbolic endpoint generation drives the memory peak

**Claim**: Symbolic edge endpoints and permutation choices in
`constrained_edges` produce most of the verification conditions; replacing them
with a three-bit topology mask and fixed endpoint pairs will let the
no-spurious proof finish within the resource cap.

**Plausibility**: High — H1 changed the edge-count constraint without changing
the 76,017 generated conditions, while symbolic endpoints and loop-generated
edge order remain.

**Prediction**: If H2 holds, the focused harness will generate materially fewer
than 76,017 conditions and return a proof verdict below 8 GiB. The model must
cover all eight simple graphs over three nodes and positive symbolic weights
for every active edge.

#### H2 Result

**Verdict**: Falsified with high confidence. Fixing the node count and endpoint
pairs while retaining all graph masks and symbolic positive weights reduced the
conditions only to 72,890; CBMC still reached the 8 GiB cap after 327 seconds.
See `/tmp/pr365-alchemist-h2-fixed-endpoints.out`.

The next experiment lowers only the harness unwind bound. Every input loop is
bounded by three nodes or three edges, so unwind four should cover the maximum
iterations plus the terminating condition while keeping unwinding assertions
enabled.

______________________________________________________________________

### H3: Unwind seven over-expands bounded adjacency loops

**Claim**: `#[kani::unwind(7)]` expands loops whose maximum body count is
three, producing unnecessary verification conditions in the no-spurious proof.

**Plausibility**: Medium — fixed endpoints changed the generated condition
count only slightly, and the current unwind bound is more than twice the
maximum graph sizes used by the proof.

**Prediction**: If H3 holds, changing only this harness to unwind four will
preserve a proof verdict and reduce solver work enough to remain under the
existing 8 GiB cap. All loop unwinding assertions must pass.

#### H3 Falsification Plan

| Step | Action                                                                                                                                                                                                              | Expected Negative Result                                                                                    |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| 1    | Temporarily change only `verify_build_adjacency_no_spurious_edges` from `#[kani::unwind(7)]` to `#[kani::unwind(4)]`, then run `./scripts/run-kani.sh decomposition verify_build_adjacency_no_spurious_edges` once. | H3 is falsified if a loop-unwinding assertion fails, or the proof still reaches the 8 GiB cap or times out. |
| 2    | Restore the staged candidate and verify its index tree and worktree are unchanged.                                                                                                                                  | A changed tree after restoration invalidates the experiment.                                                |

**Tooling**: The repository's `run-kani.sh` and its per-process limited runner;
shared Cargo cache and compiler admission remain enabled.

**Confidence on falsification**: A complete proof with unwind assertions
enabled decisively validates that four is a sufficient loop bound for this
harness. A resource failure would falsify unwind count as the main cause.

Kani documents that insufficient loop bounds fail through unwinding assertions
when those checks remain enabled; see the
[unwind attribute reference][kani-unwind].

#### H3 Result

**Verdict**: Falsified as a standalone fix. Unwind four reduced symbolic
execution to 30.8 seconds and 21,098 VCCs, but the SAT solver still reached the
8 GiB cap after 66 seconds. No proof or unwinding-assertion verdict was
produced. See `/tmp/pr365-alchemist-h3-unwind-4.out`.

The next experiment removes the nested search for each output neighbour in the
input edge list. It derives absence of spurious entries from edge preservation
and an exact total-entry count.

### H4: Nested membership searches dominate the solver formula

**Claim**: The per-output-neighbour `is_edge_in_input` search and per-node
incident-edge recount produce an unnecessarily large formula. In a graph with
unique, non-self input edges, proving every input edge appears in both
directions and the output contains exactly twice the input edge count proves
that no extra adjacency entries exist.

**Plausibility**: High — the no-spurious proof scans the input once for every
output entry after already computing each node's incident degree; other
harnesses show the edge-preservation property separately.

**Prediction**: Replacing the nested membership and incident-count assertions
with edge-preservation checks plus one total adjacency-entry count will reduce
the solver formula enough for the full three-node, three-edge proof to finish
under the existing cap.

#### H4 Falsification Plan

| Step | Action                                                                                                                                                                                                                                                                                                                                                    | Expected Negative Result                                                                   |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 1    | Temporarily rewrite only the no-spurious harness to require each input edge in both output directions, sum all output bucket lengths, and assert the sum equals `2 * edges.len()`. Keep `MAX_NODES`, `MAX_EDGES`, generated unique edges and unwind 7 unchanged. Run only `./scripts/run-kani.sh decomposition verify_build_adjacency_no_spurious_edges`. | H4 is falsified if the harness still reaches the 8 GiB cap or times out without a verdict. |
| 2    | Restore the staged candidate and verify its index tree and worktree are unchanged.                                                                                                                                                                                                                                                                        | A changed tree after restoration invalidates the experiment.                               |

**Tooling**: The repository's `run-kani.sh` and its per-process limited runner;
shared Cargo cache and compiler admission remain enabled.

**Confidence on falsification**: A proof verdict under the cap supports the
formula-size hypothesis. A memory cap failure falsifies this reduction as a
sufficient fix, although it does not rule out other solver-cost reductions.

#### H4 Result

**Verdict**: Falsified as a sufficient fix. The assertion reformulation reduced
the formula to 25,487 VCCs, but the solver still reached the 8 GiB cap after
122 seconds. See `/tmp/pr365-alchemist-h4-preservation-cardinality.out`.

The next experiment removes the dynamically allocated symbolic input edge
vector. It passes fixed-length slices for every possible three-node topology
while retaining the production adjacency builder and H4's exact-count proof.

### H5: The dynamic input edge vector drives solver memory

**Claim**: The symbolic `Vec<SimilarityEdge>` and active-count loop generated by
`constrained_edges` dominate the remaining solver state. Passing fixed length
slices for each of the eight three-node graph topologies will let the
no-spurious proof finish within the cap.

**Plausibility**: Medium — H4 substantially reduced the formula without
changing the input model, but solver memory still exceeded the cap.

**Prediction**: If H5 holds, a proof over fixed-size edge arrays and symbolic
positive weights will complete under 8 GiB. The model covers every simple
three-node topology; smaller valid node counts are embedded as graphs with
isolated trailing nodes, which do not alter adjacency for valid endpoints.

#### H5 Result

**Verdict**: Falsified. The fixed-array match for all eight topologies
generated 57,513 VCCs and still hit the 8 GiB cap after 167 seconds. See
`/tmp/pr365-alchemist-h5-fixed-edge-arrays.out`.

The combined proof appears to leave too many topology branches in one solver
problem. The next experiment isolates the largest topology in one harness; if
that fits, the final proof can give each topology its own harness.

### H6: Separate topology harnesses reduce the peak solver problem

**Claim**: Combining all eight graph-topology branches in one harness exceeds
the solver's memory budget; proving each topology in a separate harness will
keep the peak below the cap.

**Plausibility**: High — H5 expanded all eight cases into one formula and
generated 57,513 VCCs before reaching the same cap.

**Prediction**: If H6 holds, one fixed triangle topology with three symbolic
positive weights will prove under 8 GiB. It exercises the maximum bounded edge
count and all three adjacency rows.

#### H6 Result

**Verdict**: Falsified as a sufficient fix. The fixed triangle generated 8,363
VCCs but still reached the 8 GiB cap after 69 seconds. See
`/tmp/pr365-alchemist-h6-fixed-triangle.out`.

The bound-four test reduced solver work in the symbolic model. The next test
combines that bound with the fixed triangle to determine whether both
reductions together fit under the cap.

### H7: The fixed triangle proves with unwind four

**Claim**: The fixed triangle model remains over budget at unwind seven because
the bound expands loops beyond their maximum sizes; unwind four is sufficient
and will let this isolated topology prove.

**Plausibility**: Medium — H3 reduced symbolic execution but retained the full
symbolic input model; the fixed triangle has constant node and edge counts.

**Prediction**: If H7 holds, the fixed triangle with symbolic positive weights
will prove under 8 GiB and satisfy every unwinding assertion at bound four.

#### H7 Result

**Verdict**: Falsified as a sufficient fix. The fixed triangle at unwind four
generated 6,923 VCCs but hit the 8 GiB cap during propositional reduction after
50 seconds. No proof or unwinding-assertion verdict was produced. See
`/tmp/kani-focused-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-71752727.out`.

The remaining experiment holds topology and unwind constant while replacing
symbolic weights with distinct constants.

### H8: Symbolic weight bit-vectors drive propositional memory

**Claim**: Symbolic `u64` weights create solver state that exceeds the memory
cap even for a fixed triangle; concrete edge weights will allow the same
production adjacency proof to complete.

**Plausibility**: Medium — H6 and H7 used symbolic 64-bit weights and both
reached the cap even after reducing topology and unwind state.

**Prediction**: If H8 holds, the fixed triangle with weights 1, 2 and 3 will
prove under 8 GiB at unwind seven.

#### H8 Result

**Verdict**: Falsified. Concrete weights produced the same 8,363 VCCs as the
symbolic-weight case and reached the 8 GiB cap during propositional reduction
after 50 seconds. See
`/tmp/kani-focused-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-h8-24f70a7a.out`.

Both fixed-triangle runs reported one reachable `caller_location` construct and
two foreign functions. The next experiment removes the `Result::expect`
panic-formatting path and checks whether that unsupported code inflates the
proof.

______________________________________________________________________

### H9: The `expect` panic path inflates the Kani model

**Claim**: The fixed model's `Result::expect` reaches panic formatting and
unsupported caller-location or foreign-function constructs, inflating the proof
enough to exceed the memory cap.

**Plausibility**: Medium — Kani reports one reachable caller-location construct
and two foreign functions in both fixed-triangle runs; `expect` formats the
error path even though the valid endpoints make it unreachable.

**Prediction**: If H9 holds, replacing `expect` with an `assert!` on
`Result::is_ok` followed by an explicit `let Ok(...) else { return; }` will
remove the panic-formatting constructs and let the proof finish under 8 GiB.

#### H9 Falsification Plan

| Step | Action                                                                                                                                                                                                                                                                                                                                                        | Expected Negative Result                                                                                                                              |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | Temporarily use a fixed triangle with constant weights 1, 2 and 3. Replace `Result::expect` with `assert!(result.is_ok()); let Ok(adjacency) = result else { return; };`; retain production adjacency construction, H4's preservation/count assertions and unwind 7. Run only `./scripts/run-kani.sh decomposition verify_build_adjacency_no_spurious_edges`. | H9 is falsified if caller-location/foreign-function constructs remain reachable, or the harness reaches the 8 GiB cap or times out without a verdict. |
| 2    | Restore the staged candidate and verify its index tree and worktree are unchanged.                                                                                                                                                                                                                                                                            | A changed tree after restoration invalidates the experiment.                                                                                          |

**Tooling**: The repository's `run-kani.sh` and its per-process limited runner;
shared Cargo cache and compiler admission remain enabled. Kani's assertion
overrides skip string-formatting code in `assert!`, `assert_eq!` and
`assert_ne!`, making this a targeted comparison with `expect`'s panic path. See
[Kani assertion overrides][kani-overrides].

**Confidence on falsification**: A proof verdict with those constructs gone
supports the panic-path hypothesis. A resource failure falsifies this fix as
sufficient, though the unsupported path would still merit removal.

#### H9 Result

**Verdict**: Falsified. The proof still reported one `caller_location` and two
foreign functions, then reached the 8 GiB cap after 62 seconds with 17,223 VCCs
(4,618 after simplification). See
`/tmp/kani-focused-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-h9-c0b546ab.out`.

The next experiment holds the fixed triangle, weights, production builder and
explicit `Result` handling constant, replacing the search-based checks with
direct expected-row assertions.

### H10: Search-based assertions inflate the fixed-topology proof

**Claim**: Iterating over all emitted rows and searching input edges for every
adjacency entry creates the solver formula that exhausts memory. Comparing each
fixed triangle row directly with its expected entries will complete under the
cap.

**Plausibility**: Medium — H9 retained the per-edge and per-row search loops
and generated 17,223 VCCs, more than twice the fixed-triangle H8 run.

**Prediction**: If H10 holds, the direct row assertions will prove the same
fixed triangle under the existing 8 GiB cap and unwind seven.

#### H10 Falsification Plan

| Step | Action                                                                                                                                                                                                                                                                                                                                                                                           | Expected Negative Result                                                            |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| 1    | Temporarily retain the fixed triangle with weights 1, 2 and 3, production adjacency construction, H9's explicit `Result` handling and unwind 7. Replace the per-edge, total-count and nested no-spurious loops with assertions that rows 0, 1 and 2 equal their exact expected neighbour-weight slices. Run only `./scripts/run-kani.sh decomposition verify_build_adjacency_no_spurious_edges`. | H10 is falsified if the proof reaches the 8 GiB cap or times out without a verdict. |
| 2    | Restore the staged candidate and verify its index tree and worktree are unchanged.                                                                                                                                                                                                                                                                                                               | A changed tree after restoration invalidates the experiment.                        |

**Tooling**: The repository's `run-kani.sh` and its per-process limited runner;
shared Cargo cache and compiler admission remain enabled.

**Confidence on falsification**: A proof verdict supports assertion-loop
complexity as the dominant cost. Another resource failure falsifies this
assertion reduction as a sufficient fix.

#### H10 Result

**Verdict**: Falsified. Direct row assertions reduced the proof to 4,827 VCCs
(1,246 after simplification), but propositional reduction still reached the 8
GiB cap after 62 seconds. The run reported one `caller_location` and two
foreign functions. See
`/tmp/kani-focused-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-h10-4523b9ea.out`.

The remaining common cost is construction of dynamically growing adjacency
rows. The next experiment pre-counts each row's degree and reserves that
capacity before inserting edges, while keeping the H10 harness unchanged.

### H11: Repeated row-vector growth drives solver memory

**Claim**: Reallocating each adjacency row as edges are pushed creates the
solver state that exhausts memory. Counting row degrees before construction and
allocating each row once will let the fixed-triangle proof complete.

**Plausibility**: Medium — H10's direct-row proof removed the search loops but
still reached the cap with only 4,827 VCCs; the builder currently grows inner
`Vec`s through repeated `push` operations.

**Prediction**: If H11 holds, preallocated rows with the same fixed triangle
and direct assertions will prove under 8 GiB at unwind seven.

#### H11 Falsification Plan

| Step | Action                                                                                                                                                                                                                                                                                                                                                               | Expected Negative Result                                                            |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| 1    | Temporarily modify only the existing adjacency helper to validate endpoints, count incident edges per node, allocate each row with that capacity, then populate rows. Retain H10's fixed triangle, explicit `Result` handling, direct expected-row assertions and unwind 7. Run only `./scripts/run-kani.sh decomposition verify_build_adjacency_no_spurious_edges`. | H11 is falsified if the proof reaches the 8 GiB cap or times out without a verdict. |
| 2    | Restore the staged candidate and verify its index tree and worktree are unchanged.                                                                                                                                                                                                                                                                                   | A changed tree after restoration invalidates the experiment.                        |

**Tooling**: The repository's `run-kani.sh` and its per-process limited runner;
shared Cargo cache and compiler admission remain enabled.

**Confidence on falsification**: A proof verdict supports repeated allocation
as the dominant cost. Another resource failure falsifies this optimization as a
sufficient reduction.

#### H11 Result

**Verdict**: Falsified. Preallocating each row generated 5,560 VCCs (1,265
after simplification), then hit the 8 GiB process-tree cap during propositional
reduction at 66 seconds. See
`/tmp/kani-focused-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-h11-82f75b14.out`.

The next experiment retains symbolic valid graphs but checks one maximum-degree
row per harness. If a row proof fits, three such harnesses can cover all rows;
the existing preservation harness already checks every input edge in both
directions.

### H12: Per-row symbolic proofs reduce the solver formula

**Claim**: The all-row no-spurious harness combines too many row checks into
one solver problem. Proving the maximum-degree middle row independently over
all valid symbolic graphs will fit under the memory cap.

**Plausibility**: Medium — fixed-triangle direct comparisons still exhausted
memory, while the existing edge-preservation proof separately covers each input
edge. The node-one row has at most two entries in this three-node model.

**Prediction**: If H12 holds, checking every emitted entry in symbolic row 1
against the input edge set will prove under 8 GiB at unwind seven.

#### H12 Falsification Plan

| Step | Action                                                                                                                                                                                                                                                                                                                      | Expected Negative Result                                                            |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| 1    | Temporarily replace the combined no-spurious harness with one harness that keeps `symbolic_adjacency`, assumes `node_count > 1`, and checks only row 1 against all input edges. Keep the production builder and unwind 7. Run only `./scripts/run-kani.sh decomposition verify_build_adjacency_no_spurious_edges_node_one`. | H12 is falsified if the proof reaches the 8 GiB cap or times out without a verdict. |
| 2    | Restore the staged candidate and verify its index tree and worktree are unchanged.                                                                                                                                                                                                                                          | A changed tree after restoration invalidates the experiment.                        |

**Tooling**: The repository's `run-kani.sh` and its per-process limited runner;
shared Cargo cache and compiler admission remain enabled.

**Confidence on falsification**: A proof verdict supports splitting by row as a
tractable strategy. Another resource failure falsifies this split as sufficient
for the maximum-degree row.

#### H12 Result

**Verdict**: Falsified. The one-row symbolic harness still generated 25,077
VCCs (10,468 after simplification), then reached the 8 GiB cap during
propositional reduction after 91 seconds. See
`/tmp/kani-focused-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-h12-87340d96.out`.

#### Symmetry Gate Result

The full `make kani` run on staged tree
`d3bc1c351b9cf4b451900a94c82e2badb7b95b2e` also found that
`verify_build_adjacency_symmetry` reached the same resource limit. CBMC
generated 48,955 VCCs (20,710 after simplification), then hit the 8 GiB cap
after 289 seconds. See
`/tmp/kani-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-d3bc1c35.out`.

### H13: A SAT solver change reduces the in-bounds proof peak

**Claim**: Kani's supported Kissat solver will complete
`verify_build_adjacency_indices_in_bounds` within the existing 8 GiB cap.

**Prediction**: If this holds, the focused harness will produce a proof verdict
without exceeding the process-tree limit.

**Result**: Falsified. On staged tree
`55e5eb56384fc921f47c3142f089cd8f74f08c9d`, Kissat generated the same 19,611
VCCs (8,496 after simplification) and reached the 8 GiB cap after 98 seconds.
The default CaDiCaL run generated the same formula and reached the cap after 84
seconds. The exact Kissat transcript is
`/tmp/kani-kissat-09fc3177-harden-lint-support-crates-55e5eb56.out`.

### Edge-preservation gate result

On staged tree `dc33ece956169654879391a524b58b6b9b63b2ed`,
`verify_build_adjacency_preserves_edges` generated 25,092 VCCs (11,366 after
simplification), then hit the 8 GiB process-tree cap after 99 seconds. The
captured transcript is
`/tmp/kani-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-dc33ece9.out`.
The generated Proptest checks edge preservation by comparing every directed
output tuple and total output count with an independent oracle derived from the
input edges.

### Row-count gate result

On staged tree `b0a211004253f3f2429e44a24affa17df3af1120`,
`verify_build_adjacency_length` generated 16,317 VCCs (6,929 after
simplification), then hit the 8 GiB process-tree cap after 74 seconds. The
captured transcript is
`/tmp/kani-09fc3177-61fb-4b01-8e82-4b2e49d936c7-harden-lint-support-crates-b0a21100.out`.
The generated Proptest already asserts `adjacency.len() == node_count` for
each valid generated graph.

### Resolution: Check adjacency invariants with Proptest

The no-spurious, symmetry, bounds, edge-preservation, and row-count Kani
harnesses are not tractable within the repository's fixed 8 GiB process-tree
limit. Replacing no-spurious assertions, fixing its graph, preallocating rows,
isolating one symbolic row, changing the bounds solver, and reducing the
edge-preservation and row-count harnesses did not produce proof verdicts. Move
all adjacency invariants to generated property checks through the production
builder. Keep Kani enabled for the independent propagation and clone-detector
proofs.

#### Resolution Validation

1. Remove the remaining adjacency Kani model and its adjacency-only shared
   helpers.
2. Extend the generated valid-graph property to compare exact directed edge
   tuples and total output count, and to assert row-count, bounds, and
   sorted-row properties.
3. Run `make kani` and the required sequential commit gates on the exact
   candidate.

[kani-unwind]: https://model-checking.github.io/kani/reference/attributes.html
[kani-overrides]: https://model-checking.github.io/kani/overrides.html

## Recommended Execution Order

1. **Resolution** — Replace resource-intractable adjacency checks with
   generated production-builder properties, then run all gates.

______________________________________________________________________

## Termination Criteria

- **Root cause identified**: CBMC's generated Vec model exceeds the fixed
  process-tree memory cap for every adjacency property; retain their generated
  production-builder checks and keep Kani for the other proof groups.
- **Escalation trigger**: The limited runner is unavailable, a second file
   must change to run the experiment, or the result is neither a proof verdict
   nor a resource-cap/timeout failure.

## Notes for Executing Agent

The `alchemist` agent was unavailable because its selected model reported
capacity errors. Preserve the fixed runner resource limits and shared Cargo
cache. Do not treat a timeout or resource failure as a proof pass; the full
Kani gate must pass after intractable properties move to generated runtime
checks. Record the exact candidate tree and each gate's result.
