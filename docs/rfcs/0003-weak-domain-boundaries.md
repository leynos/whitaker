# RFC 0003: Weak representations at domain boundaries

## Preamble

- **RFC number:** 0003
- **Status:** Proposed
- **Repository:** `leynos/whitaker`
- **Created:** 2026-08-07
- **Updated:** 2026-08-30
- **Lint family:** Validate not, only parse
- **Family selector:** `DOMAIN`
- **Initial rules:** `discarded_parsed_value`,
  `validation_without_refinement`, and `repeated_boundary_refinement`
- **Default family status:** Experimental

## Summary

Introduce a Whitaker lint family called **Validate not, only parse** for code
that proves a domain fact at run time but fails to retain that proof in the
program's types.

This RFC addresses weak representations at domain boundaries. Its central
failure mode is **proof evaporation**:

```plaintext
weak representation
        |
        v
fallible parse or intrinsic validation
        |
        v
same weak representation continues inward
```

The first rule, `discarded_parsed_value`, detects a parsed or refined value that
is discarded while the original weak value continues to be used. The second,
`validation_without_refinement`, detects private intrinsic validators that
return only `bool` or `Result<(), E>`, after which callers continue with the
same weak value. The third, `repeated_boundary_refinement`, is a deferred
crate-wide report for weak parameters that several consumers immediately parse
or validate in the same way.

The family uses `DOMAIN` as its future Whitaker selector. Rules remain separate
lint crates so consumers can adopt them independently. The family name is a
teaching slogan; diagnostics describe concrete evidence rather than accusing
code of violating a maxim.

RFC 0004 extends the same family from boundary flow into type state spaces,
semantic partitions, sentinel encodings, primitive-to-domain projections,
invariant surfaces, and local protocol obligations.

## Problem

Rust makes it easy to encode evidence in types, but it cannot recover evidence
that code deliberately throws away. A common boundary shape accepts a broad
representation such as `String`, `u16`, `PathBuf`, or `Vec<u8>`, checks a domain
property, and then passes the original value deeper into the system:

```rust
fn enqueue(raw_url: String) -> Result<(), QueueError> {
    url::Url::parse(&raw_url)?;
    queue_raw_url(raw_url);
    Ok(())
}
```

After `Url::parse` succeeds, the function knows that `raw_url` denotes a URL.
The type system does not know this because the `Url` witness has been discarded.
Every later consumer must trust an undocumented precondition, repeat the parse,
or accept malformed data if a new call path omits the check.

The same weakness appears when a validator returns no refined value:

```rust
fn validate_port(port: u16) -> Result<(), InvalidPort> {
    if port == 0 {
        Err(InvalidPort)
    } else {
        Ok(())
    }
}

fn bind_listener(port: u16) -> Result<Listener, Error> {
    validate_port(port)?;
    Listener::bind(port).map_err(Error::from)
}
```

The success path proves `port != 0`, but the proof is represented only by
control flow. Mutation, refactoring, and additional call paths can separate the
check from the use. A newtype such as `Port(NonZeroU16)` would make the property
part of the value carried by the program.

This class of defect is architectural rather than merely stylistic:

- validation can drift away from use;
- repeated parsing adds work and inconsistent error behaviour;
- internal functions acquire undocumented preconditions;
- tests must cover omitted checks that the type system could have excluded;
- a broad representation leaks beyond the boundary where it was necessary; and
- callers cannot distinguish raw input from accepted domain data.

A useful lint must nevertheless distinguish **intrinsic** properties from
**extrinsic** facts. Syntax, ranges, non-emptiness, and correlations are usually
stable properties of a value. File existence, authorization, uniqueness,
revocation, current time, and remote service state are not. The latter cannot
normally be made permanently true by wrapping the input in a newtype.

## Current state

Whitaker already favours newtypes for domain values and asks contributors to
eliminate primitive and integer soup. Its implementation model uses one lint per
crate, shared compiler-independent analysis in `common`, localized diagnostics,
and user-interface (UI) tests for each rule.

Clippy provides useful adjacent checks, but no existing rule owns this family:

- `unnecessary_unwrap`[^clippy-unnecessary-unwrap] can replace a local
  `is_some()` plus `unwrap()` sequence
  with structured matching, but it does not ask whether a stronger receiver
  type should cross the surrounding API boundary;
- `needless_pass_by_value`[^clippy-needless-pass-by-value] analyses ownership
  rather than semantic refinement;
- `must_use` annotations can discourage ignored results but cannot establish
  that the original weak value continues inward after a successful parse; and
- compiler type checking cannot infer a nominal type from an arbitrary
  predicate over a primitive.

Whitaker should therefore target the missing architectural evidence and avoid
reimplementing local syntax checks that Clippy already handles.

## Goals and non-goals

### Goals

- Detect high-confidence cases where a parser or fallible conversion produces a
  stronger value that is discarded while its source continues to be used.
- Detect a conservative subset of private, intrinsic validators whose success
  returns no witness value.
- Identify repeated crate-local refinements that indicate a misplaced domain
  boundary.
- Explain the lost proof and recommend carrying a refined value, not merely
  moving the same validation call.
- Distinguish intrinsic value properties from extrinsic or time-varying checks.
- Preserve source spelling when that spelling is itself meaningful by allowing
  code to carry both raw and parsed forms.
- Integrate with Whitaker's future rule-code and family-selection model through
  the `DOMAIN` selector.
- Start experimentally and promote individual rules only after corpus-based
  false-positive measurement.

### Non-goals

- Infer arbitrary semantic predicates or prove general Rust functions pure.
- Ban `validate`, `check`, `verify`, `is_valid`, or similar names.
- Require every raw transport, command-line, serialization, or foreign function
  interface (FFI) type to become a domain type.
- Treat authorization, existence, liveness, uniqueness, or other extrinsic
  checks as permanent type invariants.
- Generate bespoke newtype declarations automatically.
- Rewrite public APIs without an explicit compatibility decision.
- Diagnose parsing used deliberately as one branch of format detection or
  parser selection.
- Replace Clippy rules that already provide equivalent local diagnostics.

## Terminology

### Weak representation

A type that admits values outside the domain accepted by the surrounding
operation. Examples include `String` for a validated URL, `u16` for a non-zero
port, or `Vec<u8>` for a packet whose header has already been decoded.

“Weak” is relative to a boundary. `String` is an appropriate representation for
untrusted input and a weak representation for an internal function that accepts
only normalized account identifiers.

### Refined representation

A type whose constructors establish a relevant intrinsic property and whose
safe public API preserves it. Examples include `Url`, `NonZeroU16`,
`SocketAddr`, or a domain-specific `AccountId` newtype.

### Witness value

The value returned by a successful parse or refinement. Its type carries the
fact established by the operation.

### Domain boundary

The transition between broad external or transport representations and the
narrower values used by domain logic. Boundaries commonly occur at command-line,
serialization, network, persistence, configuration, and public API ingress.

### Proof evaporation

A control-flow path establishes a proposition about a value, but the program
continues with the same pre-proof type and retains no witness that encodes the
proposition.

### Intrinsic validity

A property determined by the value itself and stable while the value remains
unchanged. Examples include a numeric range, a grammar, non-emptiness, checksum
shape, or a relationship between fields.

### Extrinsic validity

A property that depends on mutable state outside the value. Examples include
filesystem existence, current authorization, database uniqueness, certificate
revocation, or the current time.

## Family contract

### Display name and selector

The documentation name is **Validate not, only parse**. The machine-facing
family selector is `DOMAIN` because the family covers parsing boundaries,
domain state-space modelling, and local protocol surfaces that stronger APIs
can internalize.

The selector should follow the selection algebra proposed for `whitaker check`:

```plaintext
whitaker check --experimental --select DOMAIN
```

`DOMAIN` should not enter `DEFAULT` during the experimental phase. `ALL` should
include it only when experimental rules are enabled.

### Rule-code allocation

| Range | Purpose | Initial allocation |
| --- | --- | --- |
| `DOMAIN001` to `DOMAIN099` | Boundary refinement and proof retention | RFC 0003 |
| `DOMAIN101` to `DOMAIN149` | Type state spaces, semantic partitions, and invariant surfaces | RFC 0004 |
| `DOMAIN150` to `DOMAIN179` | Local protocol and lifecycle surfaces | RFC 0004 |
| `DOMAIN180` to `DOMAIN199` | Future local domain-modelling rules | Reserved |
| `DOMAIN900` to `DOMAIN999` | Workspace reports that are not ordinary Dylint diagnostics | Reserved |

_Table 1: Proposed `DOMAIN` rule-code allocation._

### Category, not monolith

The family is a documentation and selection category. It is not one broad
`validate_not_only_parse` diagnostic. Each rule receives:

- a separate lint crate;
- an independent default level and promotion decision;
- its own Fluent message set;
- focused UI fixtures; and
- shared analysis only where the evidence model genuinely overlaps.

A compiler lint group such as `whitaker::domain` can follow the shared
lint-group infrastructure. The selector does not depend on that compiler group
existing.

### Diagnostic voice

Diagnostics must report observable facts. Preferred wording includes:

```plaintext
a parsed `Url` value is discarded while the source `String` continues to be used
```

The following wording is deliberately excluded:

```plaintext
this code violates parse, don't validate
```

The first explains evidence and remediation. The second announces doctrine
without proving that it applies.

## Proposed rule: `discarded_parsed_value`

### `discarded_parsed_value` metadata

- **Rule code:** `DOMAIN001`
- **Kind:** Domain modelling
- **Initial level:** `warn` when selected experimentally
- **Analysis:** Late High-level Intermediate Representation (HIR) pass with a
  local use summary; Mid-level Intermediate Representation (MIR) confirmation
  for mutation and alias-sensitive cases
- **Promotion target:** Standard `warn` after downstream validation

### `discarded_parsed_value` rule statement

Warn when all of the following hold:

1. a recognized parse or fallible conversion consumes or borrows a weak source;
2. the successful result has a non-unit type that is stronger than the source
   representation;
3. the successful result is discarded or matched only as `_`;
4. the same source place, or a clone derived from it, continues to an inward
   use on the success path; and
5. no mutation invalidates the relationship between the parse and that use.

The rule identifies a lost witness, not merely an unused result.

### Initial refiner vocabulary

The initial recognizer should resolve definitions rather than match textual
names. It should include:

- `core::str::FromStr::from_str` and `str::parse`;
- `core::convert::TryFrom::try_from`;
- `core::convert::TryInto::try_into`;
- `core::num::NonZero*::new` constructors returning `Option<NonZero*>`;
- configured crate-local functions whose fully resolved definition paths are
  declared as refiners; and
- configured associated functions such as `crate::AccountId::parse`.

An arbitrary function returning `Result<T, E>` is not presumed to be a parser.
Authentication, resource acquisition, reservation, and remote verification can
also return values, and discarding those values does not establish the same
architectural claim.

### `discarded_parsed_value` failing example

```rust
fn enqueue(raw_url: String) -> Result<(), QueueError> {
    url::Url::parse(&raw_url)?;
    queue_raw_url(raw_url);
    Ok(())
}
```

A diagnostic should identify both the discarded `Url` expression and the later
use of `raw_url`:

```plaintext
warning[DOMAIN001]: a parsed `Url` value is discarded while its source `String` continues inward
  note: successful parsing establishes the URL grammar only in control flow
  help: bind and pass the parsed value, or carry raw and parsed forms together when source spelling matters
```

### Preferred form

```rust
fn enqueue(raw_url: String) -> Result<(), QueueError> {
    let url = url::Url::parse(&raw_url)?;
    queue_url(url);
    Ok(())
}
```

When lexical identity must be retained, both forms can be represented:

```rust
struct SubmittedUrl {
    source: String,
    parsed: url::Url,
}

impl TryFrom<String> for SubmittedUrl {
    type Error = url::ParseError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        let parsed = url::Url::parse(&source)?;
        Ok(Self { source, parsed })
    }
}
```

The lint must not assume that replacing the raw value with a canonicalized
parsed value preserves display, auditing, signature, or round-trip behaviour.
It should require retention of the witness, not destruction of the source.

### Candidate source shapes

The first implementation should recognize these success shapes:

```rust,ignore
Strong::parse(&raw)?;
raw.parse::<Strong>()?;
TryFrom::try_from(raw.clone())?;
NonZeroU16::new(port).ok_or(InvalidPort)?;

match Strong::parse(&raw) {
    Ok(_) => consume_raw(raw),
    Err(error) => return Err(error.into()),
}
```

The following shapes may join after MIR confirmation:

```rust,ignore
if Strong::parse(&raw).is_ok() {
    consume_raw(raw);
}

let accepted = Strong::parse(&raw).is_ok();
if accepted {
    consume_raw(raw);
}
```

The initial release should prefer the direct discarded-witness forms. Boolean
probe forms have a larger false-positive surface and can remain behind a
configuration flag until corpus results justify them.

### Decision matrix

| Source situation | Decision | Reason |
| --- | --- | --- |
| `Url::parse(&raw)?; consume(raw)` | Diagnose | The `Url` witness is discarded and the source continues inward |
| `let url = Url::parse(&raw)?; consume(url)` | Pass | The refined representation crosses the boundary |
| `let url = Url::parse(&raw)?; audit(raw, &url)` | Pass | The witness is retained alongside source spelling |
| Parse result used to choose among several parsers | Pass | Parsing acts as format detection rather than domain admission |
| `if !path.try_exists()? { ... }` before `open(path)` | Pass | Existence and accessibility are extrinsic and time-varying |
| Parsed source is mutated before later use | Pass or suppress | The earlier witness no longer proves a fact about the mutated value |
| Parser occurs only in a test assertion | Pass by default | The assertion may deliberately test parser acceptance |
| Parser is macro-generated and no stable source span exists | Pass | No trustworthy user-facing edit can be attributed |

_Table 2: `discarded_parsed_value` decision matrix._

### Use classification

A later source use counts as inward evidence when it:

- becomes an argument to a same-crate non-boundary function;
- is stored in a nominal domain object or long-lived field;
- is returned as the accepted result of the current boundary function;
- is sent through a channel or queue whose item type remains weak; or
- is reparsed by another consumer.

The following uses do not establish inward evidence alone:

- logging or tracing;
- construction of an error message;
- metrics labels, subject to existing low-cardinality policy;
- equality with the original input for round-trip verification; or
- retention as the source component of a value that also carries the witness.

The classifier should stay conservative when a call target cannot be resolved.

### Suggestions

A machine-applicable suggestion is allowed only for a direct statement where
binding the witness is syntactically complete and the new binding is already
consumed by an obvious next call. Most findings should use `MaybeIncorrect` help
because changing a downstream signature is an API refactor.

The lint must not synthesize a newtype, choose its visibility, invent an error
type, or silently discard source spelling.

## Proposed rule: `validation_without_refinement`

### `validation_without_refinement` metadata

- **Rule code:** `DOMAIN002`
- **Kind:** Domain modelling
- **Initial level:** `allow`, promoted to experimental `warn` for opted-in
  repositories after corpus tuning
- **Analysis:** Crate-local callee summaries plus HIR call-site collection and
  MIR success-path confirmation
- **Promotion target:** Experimental only until intrinsicness classification is
  demonstrably reliable

### `validation_without_refinement` rule statement

Warn on a crate-local validator call when all of the following hold:

1. the validator accepts a weak value by value or shared reference;
2. its success result carries no witness, normally `bool`, `Option<()>`, or
   `Result<(), E>`;
3. its body is summarized as an intrinsic, side-effect-free predicate over the
   argument or its fields;
4. the caller branches or propagates on success and then continues with the
   same weak value; and
5. no configured raw-boundary or representation-type exemption applies.

### `validation_without_refinement` failing example

```rust
fn validate_percentage(value: u8) -> Result<(), PercentageError> {
    if value <= 100 {
        Ok(())
    } else {
        Err(PercentageError(value))
    }
}

fn store_percentage(value: u8) -> Result<(), Error> {
    validate_percentage(value)?;
    repository::insert(value)?;
    Ok(())
}
```

A preferred design returns the accepted value:

```rust
struct Percentage(u8);

impl TryFrom<u8> for Percentage {
    type Error = PercentageError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        (value <= 100)
            .then_some(Self(value))
            .ok_or(PercentageError(value))
    }
}

fn store_percentage(value: u8) -> Result<(), Error> {
    let percentage = Percentage::try_from(value)?;
    repository::insert(percentage)?;
    Ok(())
}
```

### Validator summary

Names are only corroborating evidence. The initial callee summary should require
all of these properties:

- the function is defined in the current crate and is not exported;
- it takes no mutable reference and does not mutate captured or static state;
- it performs no unsafe operation;
- it makes no unresolved, virtual, foreign, asynchronous, filesystem, network,
  clock, random, database, or environment call;
- successful exits return only a boolean or unit-like success;
- rejection paths depend on comparisons, pattern tests, length, emptiness,
  character predicates, arithmetic with checked overflow handling, or calls to
  configured pure predicate helpers; and
- the argument is not transformed into another value that the caller already
  receives through a separate channel.

Unknown effects should classify the function as **not proven intrinsic** and
suppress the lint.

### Intrinsic and extrinsic examples

| Check | Initial classification | Rationale |
| --- | --- | --- |
| `value != 0` | Intrinsic | Stable while the value remains unchanged |
| `value <= 100` | Intrinsic | Numeric range property |
| `!name.is_empty()` | Intrinsic | Property of the string contents |
| `Uuid::parse_str(value).is_ok()` | Intrinsic refiner | A parser already returns a witness |
| `path.exists()` | Extrinsic | Filesystem state can change immediately |
| `authorizer.permits(user, action)` | Extrinsic | Depends on policy and mutable external state |
| `repository.is_unique(name)` | Extrinsic | Depends on concurrent database contents |
| `token.expires_at() > now()` | Extrinsic | Truth changes with time |
| `signature.verify(message, key)` | Contextual | Cryptographic evidence may merit a dedicated witness, but generic inference is unsafe |

_Table 3: Initial intrinsicness examples._

Cryptographic and capability checks should remain suppressed unless a future
rule has a domain-specific evidence model. A signature verification can be pure
in the functional sense while still proving a relationship among several
values, not an intrinsic property of one primitive.

### Diagnostics

The diagnostic should point to the validator call and the first inward use:

```plaintext
warning[DOMAIN002]: validation succeeds without producing a refined value
  note: `validate_percentage` proves an intrinsic range for `value`, but `value` remains `u8`
  help: return a domain value from the fallible boundary and accept that type internally
```

No automatic edit is proposed.

## Deferred report: `repeated_boundary_refinement`

### `repeated_boundary_refinement` metadata

- **Rule code:** `DOMAIN900`
- **Kind:** Workspace advisory report
- **Initial level:** Report-only
- **Analysis:** Crate or workspace aggregation through `whitaker check`

### `repeated_boundary_refinement` rule statement

Report a weak parameter or field shape when several internal consumers perform
the same immediate refinement or enforce the same normalized intrinsic
predicate before meaningful use.

Examples include:

- four functions accepting `&str` and beginning with `Uuid::parse_str`;
- several operations checking `port != 0` on a `u16` parameter;
- repeated `!name.is_empty()` and length bounds before accepting a `String`; or
- multiple queue consumers decoding the same packet header from `Vec<u8>`.

A single check can be algorithmic. Repeated equivalent checks across internal
entry points are stronger evidence that the boundary or parameter type is
misplaced.

### Fingerprints

The report should normalize a deliberately small predicate vocabulary:

```plaintext
NonZero(place)
Range(place, lower, upper, inclusivity)
NonEmpty(place)
LengthRange(place, lower, upper)
ParsedAs(place, resolved_target_type)
MatchesVariant(place, enum_variant)
CustomRefiner(place, resolved_def_path)
```

Equivalent commutative comparisons and range spellings should share a
fingerprint. Arbitrary closure bodies, regular expressions, and user-defined
boolean expressions should remain opaque until a later design proves a stable
normal form.

### Reporting threshold

The default threshold should require at least three distinct internal consumers
or two consumers plus one repeated validation within a receiver type. The
report should group evidence and name the common representation and refinement,
rather than emitting one warning per occurrence.

This rule belongs in `whitaker check`, not the initial Dylint pass, because it
needs crate-post aggregation and may eventually compare workspace package
boundaries.

## Shared analysis architecture

### Module layout

The shared compiler-independent model should live under a domain-specific
namespace rather than growing unrelated helpers at the `common` crate root:

```text
common/src/domain_model/
├── mod.rs
├── boundary/
│   ├── mod.rs
│   ├── model.rs
│   ├── evaluation.rs
│   ├── fingerprint.rs
│   └── diagnostics.rs
└── state_space/
    └── ... RFC 0004 ...
```

The lint crates own rustc integration:

```text
crates/
├── discarded_parsed_value/
└── validation_without_refinement/
```

The future workspace report should live behind the unified CLI rather than in a
third Dylint library.

### Pure model

A minimal boundary model should express facts rather than rustc node types:

```rust
pub enum RefinementKind {
    Parse,
    FallibleConversion,
    NonZeroConstruction,
    IntrinsicValidation,
}

pub enum WitnessDisposition {
    Retained,
    Discarded,
    Booleanized,
    Unknown,
}

pub enum SourceUse {
    Inward,
    SourceRetentionWithWitness,
    DiagnosticOnly,
    Mutated,
    Unknown,
}

pub struct RefinementEvidence {
    pub kind: RefinementKind,
    pub source_type: String,
    pub witness_type: Option<String>,
    pub witness_disposition: WitnessDisposition,
    pub later_uses: Vec<SourceUse>,
}
```

The pure evaluator should decide `Diagnose`, `Suppress(reason)`, or
`NeedsMirConfirmation`. Deterministic `BTreeMap` and `BTreeSet` collections
should preserve stable diagnostic and snapshot ordering.

### HIR prefilter

A late HIR pass should:

- resolve parser and conversion definition paths;
- obtain source and witness types from type-checking results;
- recover source-authored spans;
- collect direct local uses after the candidate expression;
- identify test, generated, FFI, and configured boundary contexts; and
- hand ambiguous mutation or alias cases to MIR confirmation.

### MIR confirmation

MIR analysis should remain local and demand a narrow fact set:

- whether the source place or any relevant projection is mutated;
- whether a reference escapes to unknown code before the inward use;
- whether the later use is reachable only through the refinement success edge;
- whether a clone consumed by the parser and the later source share the same
  underlying place; and
- whether the witness is stored through a desugared or indirect path that HIR
  did not recognize.

This work should reuse the crate-local use and escape summaries planned for the
ownership-shape phase where their contracts fit. It should not wait for a
general theorem prover or whole-program alias analysis.

### Crate-post aggregation

`repeated_boundary_refinement` should consume normalized findings emitted by
per-body analysis. Each record should contain:

- crate and item identity;
- source type;
- parameter or field position;
- normalized refinement fingerprint;
- boundary classification;
- source span; and
- whether a refined witness crossed the body boundary.

The report must sort deterministically and emit one group per common missing
boundary.

## Configuration

The canonical future configuration should use `whitaker.toml`, with equivalent
legacy loading while Whitaker retains `dylint.toml` compatibility:

```toml
[lint]
experimental = true
extend-select = ["DOMAIN"]

[lint.discarded_parsed_value]
additional-refiners = [
  "crate::account::AccountId::parse",
  "crate::wire::Packet::decode",
]
excluded-paths = [
  "crate::transport::raw",
]
booleanized-probes = false

[lint.validation_without_refinement]
additional-pure-predicates = [
  "crate::text::is_identifier_character",
]
excluded-paths = [
  "crate::serde_models",
]
```

Configuration paths must use resolved definition paths. Bare function names,
regular-expression name matching, and suffix matching are too easy to widen
accidentally.

The shared exclusion model should support:

- crate and module paths;
- individual types for deliberate raw or representation models;
- test contexts;
- generated code; and
- configured boundary functions that intentionally preserve raw source data.

Malformed paths must fail configuration or produce a bounded visible warning.
They must never collapse to a broader prefix.

## Exemptions and false-positive controls

### Raw and transport models

A broad representation is expected before parsing. Data transfer objects,
wire models, deserialization intermediates, command-line argument structs, and
FFI records should not trigger merely because they contain strings and
integers.

The lint acts only when successful refinement occurs and the value then crosses
inward without its witness. A raw type that converts once into a domain type is
the desired architecture.

### Source-preserving domains

Some systems must retain exact user input for auditing, signatures, display, or
round trips. The lint should pass when the parsed witness is stored or passed
alongside the source. Carrying both is stronger than validating and retaining
only the source.

### Parser probes

Parsing may answer “which format is this?” rather than “admit this value into
the domain”. The lint should suppress candidates where successful and failed
parses
select distinct parsers or variants and no common accepted raw path follows.

### Mutation

A witness proves a fact about particular bytes or scalar bits. Mutation between
parse and use invalidates that relationship. Such cases should suppress the
proof-evaporation diagnostic, although another lint may report stale validation.

### Extrinsic checks

Filesystem, network, clock, environment, authorization, and database checks are
out of scope. Wrapping a path in `ExistingPath` can create a dangerously stale
claim unless the wrapper owns a capability or open handle that preserves the
relevant fact.

### Macros and generated code

The lint should skip generated spans unless it can attribute an editable
source-authored expression. Macro-specific support can be added only for known
contracts whose rewrite and diagnostic locations are reliable.

## Diagnostics and localization

Each rule should provide stable Fluent keys for primary text, labels, notes, and
help. Initial slugs include:

```plaintext
discarded_parsed_value.primary
discarded_parsed_value.source_label
discarded_parsed_value.witness_label
discarded_parsed_value.note
discarded_parsed_value.help
validation_without_refinement.primary
validation_without_refinement.validator_label
validation_without_refinement.use_label
validation_without_refinement.note
validation_without_refinement.help
```

Messages must expose structured arguments for source type, witness type,
validator path, and refinement kind. English (`en-GB`), Welsh (`cy`), and
Gaelic (`gd`) bundles should land with each implemented rule.

Diagnostics should include the rule code once stable rule-code rendering exists.
Until then, the canonical lint name remains the suppression and configuration
identifier.

## Testing requirements

### Pure evaluation tests

Unit and property tests should cover:

- all combinations of witness disposition and source-use classification;
- reachability only on success and only on failure;
- mutation before and after inward use;
- deterministic fingerprint normalization;
- comparison normalization such as `x > 0` versus `0 < x`; and
- malformed configuration paths that must not broaden exclusions.

### Behaviour-driven tests

`rstest-bdd` scenarios should describe:

- a discarded parsed witness followed by inward use;
- a retained witness;
- raw and parsed forms carried together;
- source-only logging after parsing;
- parser selection among formats;
- an intrinsic unit-returning validator;
- an extrinsic filesystem or authorization check; and
- a mutation that invalidates the earlier parse relationship.

### UI tests

UI fixtures should include:

- `str::parse`, `FromStr`, `TryFrom`, `TryInto`, and `NonZero*::new`;
- direct `?`, explicit `match`, and configured parser paths;
- generic and associated parser functions;
- macro-generated calls;
- tests and doctests;
- source retention beside a witness;
- aliasing and mutation cases requiring MIR; and
- localized smoke output.

### Downstream corpus

Before promotion, the experimental rules should run across representative df12
repositories and at least one parser-heavy third-party corpus. Findings should
be classified as:

- true boundary defect;
- useful architectural advice;
- source-preservation exception;
- parser-probe exception;
- extrinsic-check misclassification;
- unresolved-call ambiguity; or
- other false positive.

Promotion should require a documented false-positive budget rather than only
passing synthetic fixtures.

## Compatibility and migration

The family is additive. Existing projects see no diagnostics unless they select
experimental rules or load an individual lint crate.

Migration normally requires API changes, so the initial rules must remain
warnings and avoid machine-applicable signature rewrites. A staged migration is
expected:

1. introduce a refined type and fallible constructor at one boundary;
2. carry it through private internals;
3. update storage or public APIs where compatibility permits;
4. retain raw source alongside the witness where required; and
5. remove repeated validators only after all call paths use the refined type.

Public libraries may keep broad compatibility entry points while delegating to a
strong internal API:

```rust
pub fn enqueue(raw_url: &str) -> Result<(), QueueError> {
    enqueue_url(url::Url::parse(raw_url)?)
}

fn enqueue_url(url: url::Url) -> Result<(), QueueError> {
    // Domain logic.
    Ok(())
}
```

Suppression should stay narrow and reasoned. A module-level exclusion is
appropriate for deliberate transport models. A crate-wide exclusion should
require a documented architecture constraint.

## Alternatives considered

### One umbrella lint

A single `validate_not_only_parse` lint could combine all evidence and emit a
variety of messages. This would make suppression, promotion, and configuration
coarse. It would also mix local parsed-value loss with speculative crate-wide
architecture. Separate rules preserve Whitaker's one-observable-phenomenon
model.

### Name-based validator detection

Matching `validate_*`, `check_*`, `ensure_*`, or `is_valid_*` would be cheap and
noisy. Names can describe extrinsic checks, side effects, or assertions, while a
poorly named intrinsic validator can omit every keyword. Resolved calls and
body summaries provide better evidence; names can enrich messages but must not
trigger a finding.

### Require `#[must_use]` on parser results

`#[must_use]` helps when the parser owner controls the API. It cannot diagnose
third-party parsers, cannot show that the original source continues inward, and
can be satisfied by `let _ = parse(...)`. It is useful hygiene but not a
replacement for proof-flow analysis.

### Rely on Clippy

Clippy addresses several local forms around unused results, unwraps, ownership,
and conversion style. It does not model the relationship among a weak source,
a discarded witness, and a later inward use. Whitaker should implement only
that residual architecture.

### Diagnose every repeated predicate

Repeated predicates can be algorithmic, performance guards, debug assertions,
or checks against changing external state. The deferred report therefore uses a
small normalized vocabulary, internal-boundary evidence, and a group threshold.
It should remain advisory until measured on real repositories.

### Add a general refinement-type language

An annotation language could let projects declare predicates and witness types.
That may become useful, but it would introduce a second type system before the
high-confidence unannotated cases have proved their value. The first release
should use Rust's existing nominal types and resolved conversion traits.

## Open questions

- Should `DOMAIN001` diagnose booleanized parser probes in its first release, or
  reserve them for `DOMAIN002` after corpus tuning?
- Which standard-library and ecosystem parser paths are safe enough for the
  built-in refiner vocabulary beyond the language traits and `NonZero*`?
- Should custom refiners use configuration only, or should Whitaker eventually
  provide a conditional attribute such as `#[whitaker::refiner]`?
- How should source-retaining containers advertise that raw and parsed forms
  remain coupled when the fields live in separate modules?
- Can the ownership-shape MIR summaries be reused directly, or does proof
  retention require a smaller dedicated place-equivalence model?
- Should `DOMAIN900` group across workspace packages, or remain crate-local in
  its first report format?

## Recommendation

Accept the `DOMAIN` family allocation and implement `DOMAIN001` first as an
experimental, high-confidence lint. Build its pure evidence model and direct HIR
path before adding MIR-only cases. Use downstream findings to calibrate the
inward-use classifier and source-preservation exemptions.

Keep `DOMAIN002` at `allow` until crate-local intrinsicness summaries have been
validated against a real corpus. Implement `DOMAIN900` only through the unified
`whitaker check` pipeline after normalized per-body findings can be aggregated
without duplicating Dylint diagnostics.

This sequencing catches the clearest form of proof evaporation first, leaves
room for lexical-source domains, and avoids turning a useful design principle
into a doctrinaire regex with a compiler badge.

## References

[^clippy-unnecessary-unwrap]:
    [Clippy `unnecessary_unwrap` documentation](https://rust-lang.github.io/rust-clippy/master/index.html#unnecessary_unwrap)

[^clippy-needless-pass-by-value]:
    [Clippy `needless_pass_by_value` documentation](https://rust-lang.github.io/rust-clippy/master/index.html#needless_pass_by_value)
