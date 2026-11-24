# Architectural decision record (ADR) 0021: Type-3 refinement semantics for SARIF run 1

## Status

Accepted (2026-11-24). The maintainer ruled on 2026-11-23 that the abstract
syntax tree (AST) pass reclassifies `WHK001` (Type-1) results by the same rule
as `WHK002` (Type-2) results, and roadmap item 7.3.2 completed the rest of this
record. Its implementation landed with the decisions below; the calibration
finding in [Calibration outcome](#calibration-outcome-the-type-3-feature) is
the one open item. Roadmap item 7.3.3 implements the follow-up set out in
[Follow-up: exact Type-1 acceptance](#follow-up-exact-type-1-acceptance).

## Date

2026-11-23.

## Context and problem statement

The clone detector runs two passes. The token pass (Run 0) accepts candidate
pairs by Jaccard similarity over winnowed fingerprint sets and reports them as
`WHK001` (Type-1) or `WHK002` (Type-2). The AST pass (Run 1) lowers each
fragment to a normalized syntax tree, compares canonical AST hashes, and scores
a cosine similarity over depth-weighted node-kind histograms. It reports
near-miss clones as `WHK003` (Type-3).

The [clone detector design](../whitaker-clone-detector-design.md) says the AST
pass should "add or merge a result under `WHK003`, or upgrade `WHK002` to
`WHK003`". It never says why `WHK001` is left out. The only reason it implies
comes from the rule definition: "`WHK001` – Type-1 clone (token exact after
trivia removal)". If Type-1 meant exact token equality, the exemption would
cost nothing. Identical tokens, identifier text included, parse to identical
trees, so the AST pass could never find a near-miss inside a true Type-1 pair.

The shipped token pass does not provide an exact Type-1. The design's own
configuration sets `type1 = 0.95`, and roadmap item 7.2.3 implemented that as a
T1-profile Jaccard of at least `19/20` over winnowed fingerprint sets. A
`WHK001` pair therefore means "at least 95% of the kept T1 fingerprints are
shared", which admits two kinds of difference:

- **Fingerprint differences.** Up to 5% of the fingerprint set may differ. In
  a long function, one added or removed statement can stay inside that margin.
- **Order and repetition.** Sets ignore how often a fingerprint occurs and
  lose ordering beyond the shingle width, so some statement reorderings keep
  the score high.

These are exactly the edits that the AST pass exists to detect. The design's
implied exemption rested on an exact Type-1 that the 0.95 threshold does not
provide.

## Decision drivers

- A result's rule must describe the code it reports. `WHK001` promises
  whitespace or comment differences only.
- One code region must not carry contradictory labels in one run. When the
  caller supplies both normalization profiles, Run 0 commonly reports the same
  physical pair as `WHK001` (T1 profile) and `WHK002` (T2 profile).
- Run 1 must stay positive-evidence only: AST evidence may add or strengthen a
  result, never remove or weaken one.
- 7.3.2 must not change Run 0 acceptance, which belongs to 7.2.3.

## Options considered

### Option A: keep `WHK001` fixed

Run 1 carries every `WHK001` result unchanged, whatever the AST evidence says.
This follows the design text literally and keeps Type-1 labels independent of
cosine calibration and parser behaviour. It mislabels any `WHK001` pair that
contains a structural edit, and the AST evidence that shows it (differing
hashes and the cosine) would sit on a result still labelled Type-1. With both
profiles present, a structurally edited pair would leave Run 1 as both
`WHK001 unchanged` and `WHK003 updated`.

### Option B: apply the `WHK002` rules to `WHK001`

Conclusive AST evidence of a near-miss, meaning differing canonical hashes and
a cosine at or above `type3`, reclassifies a token result to `WHK003` whichever
token rule produced it. A `WHK001` result that survives Run 1 is then either
confirmed by equal canonical hashes or carried because the AST evidence was
inconclusive.

### Option C: make Type-1 acceptance exact first

Restore the design's premise in the token pass, then keep `WHK001` fixed. This
is the cleanest end state, but it changes Run 0 semantics and the `type1`
configuration, which lie outside 7.3.2.

| Concern                        | Option A               | Option B                       | Option C                    |
| ------------------------------ | ---------------------- | ------------------------------ | --------------------------- |
| `WHK001` label accuracy        | Wrong for edited pairs | Correct on conclusive evidence | Correct by construction     |
| Dual labels for one region     | Possible               | Removed                        | Removed                     |
| Deviation from design text     | None                   | Run 1 rules                    | Run 0 acceptance and config |
| Exposure to cosine calibration | `WHK002` only          | `WHK001` and `WHK002`          | `WHK002` only               |
| Fits inside roadmap item 7.3.2 | Yes                    | Yes                            | No                          |

_Table 1: Trade-offs between the treatments of `WHK001` in Run 1._

## Decision outcome / proposed direction

Adopt option B in 7.3.2, and schedule option C as roadmap item 7.3.3.

Run 1's accepted-pair classification uses the same rows for both token rules:

```plaintext
token rule         AST evidence                                Run 1 result
-----------------  ------------------------------------------  ----------------------------
WHK001 or WHK002   conclusive, hashes equal                    same rule carried, unchanged
WHK001 or WHK002   conclusive, hashes differ, cosine >= type3  WHK003 reclassified, updated
WHK001 or WHK002   conclusive, hashes differ, cosine <  type3  same rule carried, unchanged
WHK001 or WHK002   inconclusive                                same rule carried, unchanged
```

A reclassified result records its Run 0 rule in
`properties.whitaker.tokenRuleId`, so a `WHK003` result may carry
`tokenRuleId: "WHK001"`. A `WHK001` pair with differing hashes but a cosine
below `type3` keeps its label, as the positive-evidence rule requires. Option B
remains correct after option C lands: the `WHK001` reclassification rows then
fire only through covering-node effects, as described below.

### Open measurement

How often `WHK001` pairs actually have differing canonical hashes is not known.
Roadmap item 7.3.2 measures it on its cosine calibration fixtures (obligation
K1) and its end-to-end test, and records the figure in its ExecPlan. The figure
sets the priority of item 7.3.3. It does not change this decision, because the
mislabelling is possible by construction whenever the rate is non-zero.

## Follow-up: exact Type-1 acceptance

Roadmap item 7.3.3 makes the token pass accept Type-1 only on exact equality,
which restores the meaning the rule's description already claims.

### Proposed change

- Accept a T1-profile candidate pair as `WHK001` only when the two fragments'
  normalized T1 token-code sequences are equal. Trivia is already removed and
  identifier, lifetime and literal text is preserved by the T1 profile, so
  sequence equality is "token exact after trivia removal".
- Keep fingerprint-set Jaccard as a pre-filter and as the reported score. An
  exact pair always has a Jaccard of `1`, so only pairs with a Jaccard of `1`
  need the sequence comparison.
- Retire the `type1` threshold. The threshold-order rule becomes
  `type2 > type3`. No compatibility alias is kept, because the clone detector
  is experimental and pre-1.0.
- A T1 pair that is near-exact but not exact is no longer reported under T1.
  The same physical pair is normally also a T2 candidate with a Jaccard at
  least as high, so it reaches Run 0 as `WHK002` and Run 1 refines it.

### Options for the exactness test

| Test                                 | Exact | Cost                               |
| ------------------------------------ | ----- | ---------------------------------- |
| T1 token-code sequence equality      | Yes   | One linear comparison per pair     |
| Fingerprint-set Jaccard equal to `1` | No    | Free, but ignores order and counts |
| Status quo: Jaccard of at least 0.95 | No    | None                               |

_Table 2: Candidate tests for Type-1 acceptance._

Sequence equality is the only exact test. Set equality alone repeats the
present defect at a smaller scale, so it serves only as the pre-filter.

### Consequences

- Run 0 semantics change: fewer `WHK001` results, more `WHK002` results. Each
  result's `whitakerFragment` identity is independent of its rule, so a rule
  change does not change the fragment identity, though a tool that keys alerts
  on rule ID sees an alert close and reopen.
- Recall can fall for a near-exact T1 pair whose T2 Jaccard is below `type2`.
  Run 1 can still add such a pair as `WHK003` from AST evidence. Item 7.3.3
  measures this on the 7.3.2 fixtures before it lands.
- The AST pass needs no change. With exact Type-1, a `WHK001` pair's requested
  spans hold identical tokens and lower to identical normalized trees, so the
  hashes differ only when the two covering nodes differ. The covering-node
  expansion guard already makes large expansions inconclusive, so the `WHK001`
  reclassification rows become a defensive check rather than a routine path.

### Verification

- A property test that T1 acceptance holds exactly when the normalized T1
  token-code sequences are equal, including generated reorderings and
  duplications that keep the fingerprint set unchanged.
- An end-to-end assertion that every `WHK001` pair in the 7.3.2 fixtures has
  equal canonical AST hashes when its covering spans equal its requested spans.
- The 7.3.2 measurement repeated, expecting zero `WHK001` pairs with
  differing hashes outside covering-node effects.

## Decisions taken by roadmap item 7.3.2

- **Exact cosine arithmetic.** Acceptance is computed over
  `num_bigint::BigUint`: `d^2 * dot^2 >= n^2 * A * B`, evaluated only after
  establishing `A > 0` and `B > 0`. Floats would make borderline decisions
  depend on rounding. `num-bigint` 0.5.1 (MIT OR Apache-2.0, MSRV 1.60) and its
  transitive `num-integer` are the only new crates. It never appears in a
  public signature.
- **Classification tables.** Table A classifies every accepted token pair and
  Table B every physical pair no profile accepted. An unaccepted pair with
  **equal** canonical hashes is added as `WHK002`, not `WHK003`: equal
  canonical trees mean the code differs only in identifiers, literals and
  types, which is Type-2 in the clone taxonomy. That answers the open question
  the plan carried as OQ2.
- **Fragment identity.** A fragment's physical location is
  `(file_uri, span)` and is independent of the normalization profile. Carried
  and reclassified results keep their Run 0 `whitakerFragment`; an added result
  is keyed by the SHA-256 of its two physical locations. Residual churn: a pair
  whose token verdict flips between commits changes its `whitakerFragment`.
- **Threshold validation.** `type1 > type2 > type3` must hold, and `type3` must
  be a six-decimal value, tested with `checked_rem`. Together with the
  printed-score proof, that guarantees the printed cosine never contradicts the
  acceptance decision. The default `type3` stays `17/20`.
- **Provenance properties.** `properties.whitaker.refinement` (`carried`,
  `reclassified`, `added`), `tokenRuleId` on reclassified results, and
  `astHashesEqual`. Whitaker's consumers read provenance from there, never from
  `baselineState`, which other SARIF tools may rewrite.
- **Inconclusive-evidence guards.** `min_nodes = 10` and
  `max_cover_expansion = 2`. Below the node floor, or when a covering node is
  more than twice the fragment's length, the pair's evidence is inconclusive
  and the token verdict stands.
- **`WHK001` differing-hash measurement.** Not yet measured on a realistic
  corpus: the K1 fixtures are function-scale pairs, and every fixture pair the
  suite exercises hashes equal when its covering span equals its requested
  span. The measurement remains an open task for item 7.3.3, which is where it
  changes a decision.

## Calibration outcome: the Type-3 feature

Obligation K1 measured three candidate features over six unrelated pairs and
three near-miss pairs of realistic Rust. Values are cosine micros.

| Feature                | Unrelated pairs    | Near-miss pairs      |
| ---------------------- | ------------------ | -------------------- |
| Depth-weighted kinds   | 984,762 to 999,590 | 998,158 to 1,000,000 |
| Unweighted kind counts | 868,092 to 982,173 | 975,809 to 1,000,000 |
| Trigram productions    | 626,212 to 929,516 | 942,074 to 1,000,000 |

_Table 3: K1 measured the cosine of each candidate feature._

The depth-weighted histogram, which the design named, cannot separate the
tables: unrelated functions with the same signature shape score above 0.98,
because shallow `PATH`/`NAME_REF`/punctuation kinds dominate a small histogram.
Run 1 therefore scores the **trigram-production** cosine.

The shipped default remains `17/20`, and K1's strict form — every unrelated
pair below the threshold — still fails for three of the six unrelated pairs.
Raising `type3` above `9/10` would require reordering the Type-2 threshold,
which is Run 0 acceptance and outside this item. The measured table is recorded
here rather than resolved by tuning, so the choice between raising both
thresholds and filtering plumbing kinds can be made on its own evidence.
