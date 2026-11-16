# Architectural decision record (ADR) 0016: `?` is a branch

## Status

Accepted (2026-11-16): `bumpy_road_function` charges the `?` operator as the
handwritten branch it abbreviates, an `if` whose body returns early. The charge
is one predicate branch, one nesting depth, and one control-flow weight on the
`?` token's line: 2.0 at the default weights. It is not zero, and it is not the
two-arm `match` that rustc lowers `?` into. Changing this requires a
superseding ADR approved by the maintainer.

## Date

2026-11-16.

## Context and problem statement

rustc lowers `value?` into a `match` over `Try::branch` with a success arm and
a residual arm that returns. Desugarings are transparent to the shared
provenance classifier
([ADR 0013](adr-0013-node-origin-provenance-classifier.md)), so the lint has
to decide what a `?` costs. It has given three different answers.

- **Whitaker 880939d (#574) over-charged.** The default walk charged each
  lowered arm a nesting depth and a control-flow weight, so one `?` cost 3.0:
  two arms, each charged depth 1.0 and flow 0.5. A flat guard chain was
  reported as nested conditional logic.
- **Whitaker 40a85aa5 (#623) charged nothing.** `visit_try` walked only the
  scrutinee, on the reasoning that the author "wrote a propagation, not a
  branch". That hid every propagation point from the lint and contradicted the
  maintainer's ruling below. The users' guide documented the opposite of the
  ruling.
- **The ruling was never recorded in this repository.** An agent working on
  #623 therefore reversed it without meeting it. This ADR is that record.

The maintainer ruled on 15 November 2026, on the estate-wide findings that
880939d raised: "`?` is a branch". On cuprum's refactor of
`consume_with_reader` in
[leynos/cuprum #714](https://github.com/leynos/cuprum/pull/714) they wrote:
"The loop now tells a story. The previously inline clauses in the story now
have clear names and declared inputs and outputs … this is exactly the type of
refactor that the bumpy road lint is designed to foster."

On 16 November 2026, after [#623](https://github.com/leynos/whitaker/pull/623)
merged, they asked for "the forward fix, with an ADR as a matter of urgency".
Asked how a `?` should be charged, they chose to charge it as the handwritten
branch it abbreviates,
`match x { Ok(v) => v, Err(e) => return Err(e.into()) }`, which they called
"the most honest option", scored by SegmentBuilder as an `if` whose body
returns early, without tuning the weights toward any fixture.

## Decision drivers

- A `?` is a place where control may leave the function. A reader must follow
  it, so the lint should see it.
- The charge must be what the written-out form earns, not what rustc's
  scaffolding happens to earn.
- CodeScene is the reference specification for the smell
  ([ADR 0011](adr-0011-bumpy-road-reference-semantics.md) and
  [RFC 0013](../rfcs/rfc-0013-bumpy-road-semantic-alignment.md)), so the new
  charge must not contradict its verdicts on `?`.
- The charge is not tuned to flag or clear any particular function. Whether a
  function is flagged is a measured outcome.

## Options considered

| Option                    | Charge per `?` at default weights        | Verdict                                      |
| ------------------------- | ---------------------------------------- | -------------------------------------------- |
| Lowered `match` (880939d) | 3.0 (two arms, depth and flow each)      | Rejected: double counts the scaffolding.     |
| Nothing (#623)            | 0.0                                      | Rejected: contradicts "`?` is a branch".     |
| One predicate only        | 0.5                                      | Rejected: ignores the early return and body. |
| Early-return `if` form    | 2.0 (predicate 0.5, depth 1.0, flow 0.5) | Chosen.                                      |

*Table 1: Charges considered for the `?` operator.*

## Decision outcome

`SegmentBuilder::visit_try` walks the scrutinee, so the operand's own authored
structure is still measured, then pushes three segments over the `?` token:

- one predicate segment of one branch, weighted by `weights.predicate`
  (0.5);
- one depth segment, weighted by `weights.depth` (1.0), standing for the body
  of the early return; and
- one flow segment, weighted by `weights.flow` (0.5), standing for the return
  that leaves the function.

The lowered arms and the generated `return` contribute nothing beyond these.
The segments sit on the `?` token rather than the whole expression, so a `?`
that ends a long method chain charges its own line only. An explicit `match`
over the same call is not a desugaring and is still charged for its two
authored arms. The `for` loop desugaring, charged once as in #623, is unchanged.

## CodeScene comparison

CodeScene (CLI 1.0.36) was run on 2026-11-16 on the samples below.

| Sample                                                                     | CodeScene                                                                    | Whitaker with this ADR                             |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | -------------------------------------------------- |
| Flat chain of twelve `?` (`pass_flat_try_chain`)                           | 10.0, no findings                                                            | Silent                                             |
| `?` run inside a loop, split form (`pass_question_mark_read_loop_split`)   | 10.0, no findings                                                            | Silent                                             |
| `?` run inside two loops, unsplit (`pass_question_mark_read_loop_unsplit`) | Bumpy Road, `bumps = 2`                                                      | Silent (structural-region gap)                     |
| `through_question` / `through_match` (`fail_try_versus_match`)             | `through_question` not a Bumpy Road; `through_match` Bumpy Road, `bumps = 2` | `through_question` silent; `through_match` flagged |
| cuprum `consume_with_reader` before #714 (1d9e7d37)                        | 10.0, not flagged                                                            | Silent (`cuprum-streams`, whole crate)             |
| cuprum after #714 (3c60048)                                                | 10.0, no findings                                                            | Silent (`cuprum-streams`, whole crate)             |

*Table 2: Verdicts of CodeScene and Whitaker on `?`-driven samples.*

Three conclusions follow.

1. CodeScene counts no nesting for `?`. Flat chains, the `?` form and the real
   pre-split function are all clean there. The charge in this ADR agrees with
   CodeScene on every `?`-driven verdict above: in none of these samples does a
   `?` push Whitaker over the threshold.
2. CodeScene's two bumps in the unsplit fixture come from its two
   `loop { if … { break } }` regions, which need two levels of conditionals,
   and not from the `?` operators. Whitaker's silence there is the
   structural-region gap recorded in RFC 0013, not a question about `?`. The
   control confirms it: the same function with every `?` removed (no `?` in the
   file) still scores 9.84 with `Bumpy Road Ahead`, `consume_with_reader`,
   `bumps = 2` (CLI 1.0.36), identical to the version with `?`.
3. cuprum #714 was flagged by 880939d's over-count only. CodeScene never
   flagged it, and Whitaker does not flag it under this ADR: a suite built from
   the commit that introduced this ADR reported no finding for `cuprum-streams`
   at 1d9e7d37 (before #714) or at 3c60048 (after it). The refactor was still
   wanted, and the maintainer's ruling on it stands: it is the shape of change
   the lint exists to encourage.

## Consequences

- A function that propagates on many adjacent lines, or inside nested
  conditionals or loops, scores higher than under #623. A flat guard chain with
  one `?` per line scores 2.0 per line and, at the default threshold of 2.5,
  stays silent unless it sits inside further nesting. Several `?` on one line
  add up: two score 4.0 on that line.
- The flow weight is the maintainer's ruling, not a property of the `if` walk.
  `SegmentBuilder` charges an authored `if` a predicate and a depth, and an
  authored `return` nothing; flow is charged only to match arms. A `?` is
  charged flow as well because its early return leaves the function, which the
  maintainer asked to be scored. An authored early-return `if` therefore scores
  1.5 against 2.0 for the `?` that abbreviates it. A superseding ADR would be
  needed to change either side.
- Estate refactors made under the ruling are intended. The lint's purpose is
  named-helper splits that tell a story, with declared inputs and outputs. Rows
  that find a `?`-heavy function flagged refactor it; they do not argue a false
  positive.
- The users' guide, developers' guide and `visit_try` module documentation now
  describe this charge and cite this ADR.
- Fixtures: `pass_flat_try_chain`, `pass_question_mark_read_loop_split` and
  `pass_question_mark_read_loop_unsplit` pin the measured outcomes above,
  `fail_question_mark_parenthesized_runs` pins the weights and the `?` token
  line, including a `?` whose expression ends on a later line, and
  `pass_two_flat_for_loops` and the other `for` fixtures are unchanged.
- Changing the `?` charge needs a superseding ADR approved by the maintainer.
