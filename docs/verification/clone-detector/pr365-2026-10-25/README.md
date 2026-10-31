# Clone-detector proof run for PR 365

This record captures exact-head Kani runs for PR 365: the published `bf9aa109`
candidate and the documentation-only `e100b422` candidate. The historical runs
and unwind-5 negative control from 2026-10-19 remain in
[`pr365-2026-10-19`](../pr365-2026-10-19/README.md); they are separate evidence.

## Follow-up execution on candidate e100b422

| Field            | Value                                      |
| :--------------- | :----------------------------------------- |
| Candidate commit | `e100b4227d815263f1abc5c769454a99bcb6b881` |
| Candidate tree   | `c67a76716874c072ce5c2ea4ee5de6b4dbee4f7b` |
| Worktree state   | Clean before and after                     |
| Command          | `make kani-clone-detector`                 |
| Tool versions    | Kani 0.67.0, CBMC 6.8.0, CaDiCaL 2.0.0     |
| Run duration     | 540 seconds                                |
| Harness result   | 15 passed; 0 failed or inconclusive        |

The run used the committed HEAD and tree above. Its complete transcript is
[`kani-clone-detector-e100.log.gz`](kani-clone-detector-e100.log.gz). The
uncompressed transcript SHA-256 is
`99766e3c8d20142bf114e30588076ba2bea85049a9b480eeba8022976fcc2923`; the
compressed archive SHA-256 is
`7cafc4a98415c71c0e73d783c4c84313f1deb0d40cb3ae58e65f49055bc4d9c3`. The largest
proof was insertion-order independence at 187 seconds and 4,835 MiB.

## Candidate and execution

| Field            | Value                                      |
| :--------------- | :----------------------------------------- |
| Candidate commit | `bf9aa109ca8157d70fc7aa030e9e8dc04fd5909d` |
| Candidate tree   | `e68c2eb57e2ed7ff0f32e73113e359c4f44720ab` |
| Comparison base  | `45b06add7c8f1fd6334377ff5c4e07931c95e38b` |
| Command          | `make kani-clone-detector`                 |
| Expanded command | `./scripts/run-kani.sh clone-detector`     |
| Tool versions    | Kani 0.67.0, CBMC 6.8.0, CaDiCaL 2.0.0     |
| Run duration     | 364 seconds                                |
| Harness result   | 15 passed; 0 failed or inconclusive        |

The worktree was clean before and after the run. Its HEAD and tree matched the
values above on both checks. The archived transcript is
[`kani-clone-detector.log.gz`](kani-clone-detector.log.gz). Its uncompressed
SHA-256 is `c8b0c7e720df50f30ef6b9ff0c1c5abf7ea85ca3fc265466951499e31d3d6bae`;
the compressed file SHA-256 is
`f07f73c807e18284ae1618e82fae60d3b5f1404fc43862e4ce4a2e2a3b7cf692`.

Each proof ran sequentially with an 8 GiB process-tree memory cap, swap
disabled, and a 600-second timeout. A timeout, out-of-memory result, non-zero
exit, or missing peak measurement is unsuccessful or inconclusive. The largest
run was insertion-order independence at 117 seconds and 4,857 MiB.

The proof-source patch from the previously assessed base is reproducible with
this exact generation command. Explicitly set `--abbrev=8` so Git emits stable
index abbreviations across environments:

```sh
git diff --binary --no-ext-diff --no-textconv --abbrev=8 \
  a20569eabd350954f639bed5b8307a2435e16070 \
  bf9aa109ca8157d70fc7aa030e9e8dc04fd5909d -- \
  crates/whitaker_clones_core/src/ast/kani.rs \
  scripts/run-kani.sh > /tmp/kani-proof-source-a205-to-bf9.patch
sha256sum /tmp/kani-proof-source-a205-to-bf9.patch
```

The patch SHA-256 is
`e3c44ae90da6fe34bb179af1803ab4d5adc636df34f8cb73d20ac7c2d270b593`. This
command and patch identify the exact AST proof and runner changes used by both
candidates; the index harness source was already present at the comparison
base. The `e100b422` candidate leaves those proof sources unchanged.

## Harness results

| Harness                                                  | Result | Duration | Peak memory |
| :------------------------------------------------------- | :----- | -------: | ----------: |
| `verify_lsh_config_new_smoke`                            | passed | 2 s      | 95 MiB      |
| `verify_lsh_config_new_symbolic`                         | passed | 1 s      | 68 MiB      |
| `verify_lsh_config_new_overflow_product`                 | passed | 1 s      | 68 MiB      |
| `verify_min_hasher_sketch_rejects_empty_input`           | passed | 9 s      | 236 MiB     |
| `verify_min_hasher_lane_minimum_for_boundary_hashes`     | passed | 2 s      | 68 MiB      |
| `verify_min_hasher_unique_hashes_sorts_and_deduplicates` | passed | 11 s     | 163 MiB     |
| `verify_lsh_index_rejects_self_pairs`                    | passed | 71 s     | 1,941 MiB   |
| `verify_lsh_index_canonicalizes_pair_order`              | passed | 65 s     | 2,884 MiB   |
| `verify_lsh_index_deduplicates_repeated_band_collisions` | passed | 77 s     | 3,584 MiB   |
| `verify_lsh_index_is_insertion_order_independent`        | passed | 117 s    | 4,857 MiB   |
| `verify_smallest_covering_node_selects_minimal_range`    | passed | 2 s      | 158 MiB     |
| `verify_missing_cover_returns_none`                      | passed | 2 s      | 68 MiB      |
| `verify_kind_id_round_trips_full_u16_domain`             | passed | 0 s      | 68 MiB      |
| `verify_kind_counts_stop_at_max_depth`                   | passed | 4 s      | 154 MiB     |
| `verify_kind_id_equality_and_order_cover_small_alphabet` | passed | 0 s      | 68 MiB      |

## Interpretation limits

- The covering harness checks three symbolic candidate ranges in offsets
  `0..=16`, assuming at least one covers the target. It checks shared
  production selection predicates, not parser candidate enumeration. The
  missing-cover harness checks an empty array and one fixed non-covering array.
- The `KindId` round trip covers all 65,536 `u16` values. The depth harness
  checks one five-node fixed-kind tree with a fixed-array tally. The four-value
  ordering harness covers its small symbolic alphabet only. These checks do not
  prove general traversal fidelity or production `BTreeMap` accumulation;
  runtime and property tests cover production accumulation and ordering.
- The configuration harnesses call production `LshConfig::new`. The symbolic
  case bounds both inputs to `[0, 128]`; the overflow case fixes `rows = 2` and
  assumes `bands > usize::MAX / 2`. The smoke case checks one accepted input.
- The MinHash checks cover empty-input rejection, a lane minimum for fixed
  mixed hashes `0` and `u64::MAX`, and normalization of four fixed
  fingerprints. The final harness checks sorted unique **fingerprint hashes**
  through `unique_hashes`; it does not check complete sketches or signatures.
- The four LSH pair proofs use a separate bounded Kani model with at most four
  fragments, two bands, and six recorded pairs. It does not execute production
  bucket enumeration or the production cross-band set deduplication. The
  canonical-order harness uses `CandidatePair::new` for both the modelled pair
  and expected pair, so it checks reversal invariance but is not an independent
  oracle for that constructor's ordering rule. The repeated-band harness
  summarizes a pair once when any modelled band matches; it does not simulate
  repeated production bucket emissions. Runtime tests and BDD scenarios cover
  the public production path.

The 2026-10-19 unwind-5 negative control remains in the historical record. It
failed the loop-termination unwinding assertion; the current depth harness uses
unwind 6 with unwinding assertions enabled. That control demonstrates the
importance of the loop bound, not a mutation test of the depth tally.
