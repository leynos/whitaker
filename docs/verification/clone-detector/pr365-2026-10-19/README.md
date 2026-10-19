# Clone-detector proof run for PR 365

This directory preserves the bounded model-checking output used to assess the
clone-detector proofs in PR 365. The complete Kani output includes every
harness transcript and its per-process-tree resource result.

## Kani

The full run used Kani Rust Verifier 0.67.0 and CBMC 6.8.0 on 2026-10-19. It ran
`make kani-clone-detector`, which invokes
`scripts/run-kani.sh clone-detector`. All 15 harnesses reported
`scope_result=success` and `VERIFICATION:- SUCCESSFUL`.

Each proof process had an 8 GiB memory limit with swap disabled and a
600-second timeout. A memory or timeout limit produces an unsuccessful,
inconclusive run; it cannot be reported as a pass. The largest peak was 4,835
MiB for `verify_lsh_index_is_insertion_order_independent`, which took 159
seconds. `verify_kind_counts_stop_at_max_depth` took 10 seconds and used 151
MiB. `verify_kind_id_round_trips_full_u16_domain` took 5 seconds and used 67
MiB. In this full-group run, `verify_missing_cover_returns_none` took 2 seconds
and used 67 MiB.

The full transcript is in
[`kani-clone-detector.log.gz`](kani-clone-detector.log.gz). The focused
transcript is in
[`kani-missing-cover-focused.log.gz`](kani-missing-cover-focused.log.gz); it
was the standalone run completed before the full group. It passed in 68 seconds
and peaked at 124 MiB. This is the pre-suite measurement; the 2-second result
above is the later run within the full group. The focused invocation was
produced by
`./scripts/run-kani.sh clone-detector-harness verify_missing_cover_returns_none`

The unwind-bound negative control is in
[`kani-depth-unwind-5-negative-control.log.gz`](kani-depth-unwind-5-negative-control.log.gz).
At `#[kani::unwind(5)]`, Kani failed the loop-termination unwinding assertion
after five visits: 1 of 570 checks failed and 569 were undetermined. The run
took 43 seconds and peaked at 100 MiB. The production harness therefore uses
unwind 6; unwinding assertions remain enabled.

The runs used base commit `a20569eabd350954f639bed5b8307a2435e16070` with the
following proof-source patch applied to
`crates/whitaker_clones_core/src/ast/kani.rs` and `scripts/run-kani.sh`:

```text
SHA-256: e3c44ae90da6fe34bb179af1803ab4d5adc636df34f8cb73d20ac7c2d270b593
```

This identifies the exact source changes used by the local proof runs; the
documentation edits do not affect the compiled harnesses.

### Focused KindId and depth rerun

After merging the PR candidate changes, the three separated obligations were
rerun individually before starting broader gates. Each invocation used the 8
GiB process-tree memory cap, disabled swap, and 600-second timeout from
`scripts/run-kani-limited.sh`; all three returned `VERIFICATION:- SUCCESSFUL`
with unwinding assertions enabled.

| Harness                                                  | Duration   | Peak memory |
| -------------------------------------------------------- | ---------: | ----------: |
| `verify_kind_counts_stop_at_max_depth`                   | 23 seconds | 299 MiB     |
| `verify_kind_id_round_trips_full_u16_domain`             | 8 seconds  | 67 MiB      |
| `verify_kind_id_equality_and_order_cover_small_alphabet` | 3 seconds  | 67 MiB      |

These focused runs used Kani 0.67.0 and CBMC 6.8.0 on 2026-10-19. Their
complete outputs, including the per-process-tree resource records, are retained
in `/tmp/kani-depth-harness-20261019-rerun.out`,
`/tmp/kani-kind-id-roundtrip-20261019.out`, and
`/tmp/kani-kind-id-small-alphabet-20261019.out` in the originating workspace.
An earlier invocation exited before proof generation because the merged
`common/Cargo.toml` contained a duplicate `temp-env` entry; that manifest error
was removed before these successful runs.

## Verus

The related Verus run record is preserved in
[`verus-clone-detector.log`](verus-clone-detector.log), with the tool version in
[`verus-version.txt`](verus-version.txt). It records 9, 10, and 8 verified
proofs with zero errors, using Verus 0.2026.03.17.a96bad0 and toolchain 1.94.0.
This earlier run was performed on commit
`c59826254eccb56bce0c7290cc62ec5910d27dad`; it is identified separately from
the current Kani run.
