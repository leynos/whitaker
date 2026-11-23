# Architectural decision record (ADR) 0021: Type-3 refinement semantics for SARIF run 1

## Status

Proposed (2026-11-23). The maintainer has ruled on one point: the abstract
syntax tree (AST) pass reclassifies `WHK001` (Type-1) results by the same rule
as `WHK002` (Type-2) results. Roadmap item 7.3.2 completes the rest of this
record and moves it to Accepted when the implementation lands. Roadmap item
7.3.3 implements the follow-up set out in
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

## Outstanding decisions

Roadmap item 7.3.2 adds these before this record is accepted:

- the exact cosine arithmetic and its dependency;
- the full Run 1 classification tables, including the rule for an unaccepted
  pair with equal canonical hashes;
- fragment identity across runs, and the residual churn when a pair's token
  verdict flips between commits;
- threshold validation;
- the provenance properties (`refinement`, `tokenRuleId` and
  `astHashesEqual`);
- the inconclusive-evidence guards (`min_nodes` and the covering-node
  expansion bound);
- the measured rate of `WHK001` pairs with differing hashes.
