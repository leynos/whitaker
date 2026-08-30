# RFC 0004: Fragile types and invalid-state encodings

## Preamble

- **RFC number:** 0004
- **Status:** Proposed
- **Repository:** `leynos/whitaker`
- **Created:** 2026-08-07
- **Lint family:** Validate not, only parse
- **Family selector:** `DOMAIN`
- **Initial rules:** `manual_tagged_union`, `correlated_optional_fields`,
  `mutually_exclusive_bool_fields`, `bypassable_type_invariant`, and
  `invalid_default`
- **Default family status:** Experimental

## Summary

Extend the **Validate not, only parse** family with lints for **fragile types**:
nominal Rust types whose legal domain states form a strict subset of the states
that their fields, constructors, and mutation API can represent.

The proposal targets five observable shapes:

- a discriminator plus optional payloads that manually encode an enum;
- optional fields whose presence states are constrained by an exclusive-or,
  implication, or exactly-one rule;
- boolean fields used as a mutually exclusive state machine;
- an intrinsic invariant advertised by validation or fallible construction but
  bypassable through public construction or mutation; and
- a `Default` value that the type's own intrinsic validator provably rejects.

The first three rules recover sum types from field-correlation evidence. The
last two compare a type's invariant surface with its construction surface. All
rules begin experimentally, report on the type definition with secondary
spans at enforcement sites, and avoid automatic public-API rewrites.

RFC 0003 allocates the `DOMAIN` selector and the common distinction between
intrinsic and extrinsic validity. This RFC uses rule codes `DOMAIN101` onward
for type state-space findings.

## Problem

A Rust `struct` represents the Cartesian product of its fields. That is exactly
right when fields vary independently. It is a poor fit when the domain permits
only selected combinations.

For example:

```rust
struct Message {
    kind: MessageKind,
    text: Option<String>,
    bytes: Option<Vec<u8>>,
}
```

The representation admits combinations such as:

```plaintext
kind = Text,   text = None,    bytes = None
kind = Text,   text = Some(_), bytes = Some(_)
kind = Binary, text = Some(_), bytes = None
```

If only text messages with text payloads and binary messages with byte payloads
are legal, most representable values are invalid. Consumers must repeatedly
reconstruct the missing relationship:

```rust
match (&message.kind, &message.text, &message.bytes) {
    (MessageKind::Text, Some(text), None) => render_text(text),
    (MessageKind::Binary, None, Some(bytes)) => render_binary(bytes),
    _ => unreachable!("invalid message state"),
}
```

The enum representation expresses the actual sum directly:

```rust
enum Message {
    Text(String),
    Binary(Vec<u8>),
}
```

Similar fragility appears when:

- two `Option` fields mean “exactly one credential mechanism”;
- three booleans mean “connecting, connected, or disconnected”;
- a range type exposes public `start` and `end` fields but later asserts
  `start <= end`;
- a type offers `validate(&self)` while public fields and setters can bypass the
  same invariant; or
- `Default::default()` constructs a value rejected by `validate()`.

These designs create a persistent tax:

- every consumer must remember field correlations;
- invalid arms become `unreachable!()`, panic, or late error paths;
- mutation can cross invalid intermediate states;
- derived traits such as `Default` and `Deserialize` can silently widen the
  construction surface;
- tests must enumerate combinations that a sum type could exclude at compile
  time; and
- refactoring one state requires edits across tags, options, matches, and
  validators.

The compiler cannot warn merely because a struct has several fields. Many valid
models contain independent booleans and options. Whitaker therefore needs
**correlation evidence**, not aesthetic suspicion.

## Current state

Whitaker's repository guidance already recommends newtypes for domain values,
fallible construction for bespoke validation, and enums where variants express
meaningful alternatives. The suite also follows a conservative pattern for
structural lints: report observable shape, avoid inferring intent, and keep
separate rules independently configurable.

Clippy provides adjacent coverage:

- `struct_excessive_bools`[^clippy-struct-excessive-bools] warns when a struct
  contains many boolean fields and
  explicitly notes that an enum may represent a state machine more safely;
- `fn_params_excessive_bools`[^clippy-fn-params-excessive-bools] addresses
  boolean-heavy function signatures;
- `unnecessary_unwrap`[^clippy-unnecessary-unwrap] improves local `Option` and
  `Result` control flow; and
- `new_without_default`[^clippy-new-without-default] encourages `Default` for
  zero-argument constructors.

Whitaker should not duplicate those triggers. In particular,
`mutually_exclusive_bool_fields` must never fire merely because a struct has a
configured number of booleans. It should require evidence that particular
booleans are mutually exclusive or exhaustive. Likewise, `invalid_default`
must not oppose `new_without_default` categorically. It should report only when
a concrete default value contradicts a proven intrinsic invariant.

## Goals and non-goals

### Goals

- Detect manual sum types from discriminator and payload-use correlations.
- Detect exactly-one, at-most-one, and implication relationships among optional
  fields when code already enforces those relationships.
- Detect boolean state machines from semantic correlation evidence rather than
  field count.
- Compare intrinsic invariant checks with public construction and mutation
  paths that bypass them.
- Prove a conservative subset of invalid defaults using direct constant and
  predicate evidence.
- Point diagnostics at the nominal type while showing the code that repeatedly
  repairs or rejects its invalid states.
- Recommend enums, newtypes, private fields, fallible constructors, atomic
  mutators, or typestate according to the observed shape.
- Treat raw, builder, serialization, FFI, and runtime-resource types as
  first-class exception models.
- Reuse the `DOMAIN` family selection and rollout model from RFC 0003.

### Non-goals

- Ban structs containing several `Option` or `bool` fields.
- Infer the complete semantic state machine of arbitrary programs.
- Require all runtime state transitions to become compile-time typestate.
- Diagnose temporary partial states inside builders, decoders, parsers, or
  transactional mutation scopes merely because they are incomplete.
- Treat time-varying resource state as a permanent intrinsic invariant.
- Rewrite a public struct into an enum automatically.
- Declare a default semantically invalid without a mechanically linked
  validator or fallible constructor.
- Duplicate `struct_excessive_bools`, `fn_params_excessive_bools`,
  `unnecessary_unwrap`, or other Clippy rules.
- Analyse cross-crate consumers in the first release.

## Terminology

### Representable state space

The set of values that safe Rust code can construct through a type's visible
fields, constructors, trait implementations, and mutation methods.

For a struct with independent fields, this is approximately the Cartesian
product of each field's values, reduced by privacy and constructor logic.

### Legal state space

The subset of representable values accepted by the type's domain operations and
intrinsic invariant checks.

### Fragile type

A nominal type whose legal state space is narrower than its exposed
representable state space, such that callers or consumers must repeatedly
re-establish field correlations or preconditions.

“Fragile” describes the API contract, not the competence of its author.

### Correlation evidence

Control flow that treats combinations of fields as valid, invalid, unreachable,
or variant-specific. Examples include tuple matches, implication guards,
assertions, and invariant errors involving two or more fields.

### Construction surface

Every safe path by which downstream code can create or mutate a value:

- public named or tuple fields;
- public constructors;
- `Default`;
- `From` and `TryFrom` implementations;
- `Deserialize` implementations;
- builders;
- setters and direct mutable field access; and
- public struct update syntax where fields are visible.

### Raw representation type

A deliberately broad type used to receive transport, serialized, foreign, or
partially assembled data before fallible conversion into a domain type.

### Runtime state

A state that depends on live resources, concurrency, protocol progress, or
external effects. Runtime states may still benefit from enums, but they are not
necessarily permanent value invariants.

## Rule catalogue

| Rule code | Canonical name | Initial level | Primary evidence |
| --- | --- | --- | --- |
| `DOMAIN101` | `manual_tagged_union` | Experimental `warn` | Discriminator selects optional payload fields |
| `DOMAIN102` | `correlated_optional_fields` | Experimental `warn` after tuning | Match or guard rejects presence combinations |
| `DOMAIN103` | `mutually_exclusive_bool_fields` | `allow`, Clippy-first | Boolean combinations are rejected or unreachable |
| `DOMAIN104` | `bypassable_type_invariant` | `allow` | Intrinsic invariant plus an exposed bypass path |
| `DOMAIN105` | `invalid_default` | Report-only initially | Default is provably rejected by the same invariant |

_Table 1: Proposed fragile-type rules in the `DOMAIN` family._

The codes are grouped separately from RFC 0003 boundary rules so selectors can
later distinguish `DOMAIN0` and `DOMAIN1` prefixes if Whitaker adopts numeric
prefix selection.

## Proposed rule: `manual_tagged_union`

### `manual_tagged_union` metadata

- **Rule code:** `DOMAIN101`
- **Kind:** Domain modelling
- **Initial level:** `warn` when selected experimentally
- **Analysis:** Crate-wide HIR collection over one nominal type and its local
  consumers
- **Promotion target:** Standard `warn` after high-confidence fixture and corpus
  validation

### `manual_tagged_union` rule statement

Warn on a struct when all of the following hold:

1. it contains a discriminator field whose type is an enum, boolean, or small
   integer-like tag;
2. it contains two or more payload candidates, normally `Option<T>` fields;
3. local code branches on the discriminator and accesses a distinct payload
   subset for each tag value;
4. mismatched tag and payload combinations lead to an error, panic,
   `unreachable!()`, or an otherwise impossible arm; and
5. the mapping is stable across at least one exhaustive high-confidence match
   or two independent enforcement sites.

The rule reports a struct that behaves like an enum with associated data.

### `manual_tagged_union` failing example

```rust
#[derive(Clone, Copy)]
enum MessageKind {
    Text,
    Binary,
}

struct Message {
    kind: MessageKind,
    text: Option<String>,
    bytes: Option<Vec<u8>>,
}

impl Message {
    fn render(&self) -> Rendered {
        match (self.kind, self.text.as_deref(), self.bytes.as_deref()) {
            (MessageKind::Text, Some(text), None) => render_text(text),
            (MessageKind::Binary, None, Some(bytes)) => render_binary(bytes),
            _ => unreachable!("invalid message state"),
        }
    }
}
```

Preferred representation:

```rust
struct Message {
    body: MessageBody,
}

enum MessageBody {
    Text(String),
    Binary(Vec<u8>),
}
```

A wrapper struct remains appropriate when metadata is common to all variants:

```rust
struct Message {
    id: MessageId,
    received_at: Timestamp,
    body: MessageBody,
}
```

The lint should not suggest flattening unrelated common fields into every enum
variant.

### Discriminator classification

The initial discriminator set should include:

- local enum fields;
- `bool` fields when each value selects a distinct payload;
- integer or string tags only when matched against a closed set of constants in
  an exhaustive local match; and
- nested discriminators reached through a transparent local wrapper.

An unconstrained integer compared in one `if` statement is insufficient. The
rule needs a closed mapping between tag values and payload states.

### Payload classification

The initial payload set should include:

- `Option<T>` fields;
- nullable pointer wrappers recognized by configuration;
- empty-versus-non-empty collections only when the code explicitly treats
  emptiness as absence; and
- fields wrapped in a local transparent newtype around `Option<T>`.

Sentinel integers, empty strings, and magic values should remain out of scope in
the first release. Those representations need a separate sentinel-value design.

### Mapping evidence

The collector should normalize each branch into a mapping such as:

```plaintext
Tag(Text)   -> Required(text), Forbidden(bytes)
Tag(Binary) -> Forbidden(text), Required(bytes)
```

Evidence is high confidence when:

- every discriminator variant is covered;
- each variant has one consistent required payload set;
- the catch-all branch rejects or marks impossible all other combinations; and
- no valid branch accepts overlapping discriminator meanings.

Evidence is medium confidence when two partial matches imply the same mapping
without one exhaustive site. Medium-confidence findings should remain at
`allow` until corpus tuning demonstrates value.

### Decision matrix

| Source situation | Decision | Reason |
| --- | --- | --- |
| Enum tag selects one of two `Option` payloads; `_` is unreachable | Diagnose | A complete associated-data enum is encoded manually |
| Tag selects payload but all combinations are accepted | Pass | Fields may be independent metadata rather than an invariant |
| Tag and payload belong to a serialization-only raw type converted once | Pass | The broad product is confined to a deliberate boundary |
| Common metadata plus one tagged payload group | Diagnose payload group | An inner enum can preserve common fields |
| Integer tag matched against two constants but other values continue normally | Pass | The tag domain is not proven closed |
| External trait requires the struct field layout | Pass or suppress | The representation is imposed by an external contract |
| `#[repr(C)]` record mirrors a foreign tagged union | Pass by default | FFI layout is an explicit representation boundary |

_Table 2: `manual_tagged_union` decision matrix._

### `manual_tagged_union` diagnostic

The primary span should cover the struct name. Secondary labels should mark the
discriminator, payload fields, and strongest enforcement match:

```plaintext
warning[DOMAIN101]: `Message` manually encodes a tagged union
  label: `kind` selects which payload is valid
  label: these optional fields carry variant-specific data
  note: local matches reject tag and payload combinations that the struct can represent
  help: move associated payloads into enum variants and keep common metadata outside the enum
```

No automatic rewrite should be emitted. Visibility, derives, serialization
shape, and public compatibility make even a simple transformation non-local.

## Proposed rule: `correlated_optional_fields`

### `correlated_optional_fields` metadata

- **Rule code:** `DOMAIN102`
- **Kind:** Domain modelling
- **Initial level:** `allow`, promoted to experimental `warn` after corpus
  tuning
- **Analysis:** HIR field-correlation collection with a pure truth-table model
- **Promotion target:** Experimental or standard according to false-positive
  rate

### `correlated_optional_fields` rule statement

Warn when two or more optional fields participate in a stable presence
constraint that code repeatedly enforces, but the struct exposes the full
independent product of those fields.

Initial constraints are:

- **exactly one:** one field must be `Some` and all peers `None`;
- **at most one:** zero or one field may be `Some`;
- **all or none:** fields must be present or absent together; and
- **implication:** presence of one field requires presence or absence of
  another.

### Failing example: exactly one

```rust
struct Credentials {
    password: Option<Password>,
    token: Option<Token>,
}

impl Credentials {
    fn authenticate(&self) -> Result<User, AuthError> {
        match (&self.password, &self.token) {
            (Some(password), None) => authenticate_password(password),
            (None, Some(token)) => authenticate_token(token),
            (Some(_), Some(_)) => Err(AuthError::ConflictingCredentials),
            (None, None) => Err(AuthError::MissingCredentials),
        }
    }
}
```

Preferred representation:

```rust
enum Credentials {
    Password(Password),
    Token(Token),
}
```

### Failing example: implication

```rust
struct SessionRecord {
    authenticated: bool,
    user_id: Option<UserId>,
}

impl SessionRecord {
    fn validate(&self) -> Result<(), SessionError> {
        match (self.authenticated, self.user_id.as_ref()) {
            (true, Some(_)) | (false, None) => Ok(()),
            (true, None) => Err(SessionError::MissingUser),
            (false, Some(_)) => Err(SessionError::AnonymousUserPresent),
        }
    }
}
```

This example may be diagnosed by `manual_tagged_union` or
`correlated_optional_fields`, but Whitaker should emit at most one primary
finding for a type. The strongest applicable rule wins:

1. `manual_tagged_union` when a discriminator selects associated payload;
2. `correlated_optional_fields` for presence relationships without a clear tag;
3. `mutually_exclusive_bool_fields` for boolean-only state; and
4. `bypassable_type_invariant` for remaining constructor-surface mismatches.

### Truth-table extraction

The collector should recognize:

- tuple matches over `Option` fields;
- `matches!` expressions over option tuples;
- conjunctions of `is_some()` and `is_none()`;
- guard clauses that return `Err`, panic, or terminate for invalid
  combinations;
- `assert!` and `debug_assert!` predicates over persistent fields; and
- configured predicate helpers whose pure summary is already available.

The pure model should represent a presence row as a bit vector. For two fields:

```plaintext
password token
   0       0
   0       1
   1       0
   1       1
```

Accepted and rejected rows can then be compared with the known constraints.

### Evidence threshold

One exhaustive match that classifies every row is sufficient. Otherwise, the
initial rule should require two independent sites that imply the same
constraint. A constructor plus one consumer can count as two sites when they
are distinct bodies.

The rule should not fire when only one function happens to require a subset of
otherwise legal states. The evidence must describe the type's invariant, not a
particular operation's precondition.

### Independent options that must pass

```rust
struct SearchOptions {
    before: Option<Timestamp>,
    after: Option<Timestamp>,
    author: Option<UserId>,
}
```

Several options can be independently optional even when one query method
rejects a particular combination for performance or backend limitations. No
finding should be emitted without stable type-level correlation evidence.

### `correlated_optional_fields` diagnostic

```plaintext
warning[DOMAIN102]: optional fields in `Credentials` are not independent
  note: local code accepts exactly one of `password` and `token`
  help: represent the alternatives as enum variants, or expose only a fallible
        constructor that enforces the relationship
```

No automatic edit is proposed.

## Proposed rule: `mutually_exclusive_bool_fields`

### `mutually_exclusive_bool_fields` metadata

- **Rule code:** `DOMAIN103`
- **Kind:** Domain modelling
- **Initial level:** `allow`
- **Analysis:** HIR correlation evidence, with a mandatory Clippy-overlap gate
- **Promotion target:** Experimental only if residual findings prove useful

### `mutually_exclusive_bool_fields` rule statement

Warn when named boolean fields form a one-hot or otherwise mutually exclusive
state set, as established by explicit rejection, assertions, or exhaustive
matching.

The rule must not trigger on field count. That territory belongs to Clippy's
`struct_excessive_bools`.

### `mutually_exclusive_bool_fields` failing example

```rust
struct ConnectionState {
    connecting: bool,
    connected: bool,
    disconnected: bool,
}

impl ConnectionState {
    fn assert_valid(&self) {
        assert_eq!(
            usize::from(self.connecting)
                + usize::from(self.connected)
                + usize::from(self.disconnected),
            1,
        );
    }
}
```

Preferred representation:

```rust
enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}
```

Two booleans can also encode a fragile state even below Clippy's usual count
threshold:

```rust
struct JobState {
    running: bool,
    finished: bool,
}

fn status(state: &JobState) -> Status {
    match (state.running, state.finished) {
        (true, false) => Status::Running,
        (false, true) => Status::Finished,
        _ => unreachable!("job states are exclusive"),
    }
}
```

### Initial constraint vocabulary

The detector should recognize:

- pairwise `!(a && b)` assertions;
- a sum of boolean-to-integer conversions constrained to zero, one, or at most
  one;
- exhaustive tuple matches where selected rows are valid and peers reject;
- chained guards that reject each conflicting pair; and
- constructors that set exactly one flag while consumers assume the same rule.

It should not infer mutual exclusion from naming, such as `enabled`, `verbose`,
and `recursive`, or from the fact that several booleans appear in one
conditional.

### Clippy-first gate

Before implementation, run `struct_excessive_bools` over the candidate corpus.
Whitaker should retain only findings where semantic evidence adds material
value,
including:

- two-field state encodings below the configured Clippy threshold;
- boolean groups embedded among many unrelated fields;
- diagnostics that can identify the exact correlated subset; and
- evidence spans showing the invalid combinations already rejected by code.

If the residual corpus is small or unhelpful, `DOMAIN103` should remain recorded
but unimplemented.

## Proposed rule: `bypassable_type_invariant`

### `bypassable_type_invariant` metadata

- **Rule code:** `DOMAIN104`
- **Kind:** Domain modelling
- **Initial level:** `allow`
- **Analysis:** Type-level construction-surface inventory plus intrinsic
  invariant summary
- **Promotion target:** Experimental after public-API and raw-type suppression
  settle

### `bypassable_type_invariant` rule statement

Warn when a type has a mechanically identified intrinsic invariant and also
exposes a safe construction or mutation path that can bypass that invariant.

The rule requires both halves:

1. **Invariant evidence**, such as a fallible constructor, `TryFrom`, or pure
   validator that rejects field values; and
2. **Bypass evidence**, such as public fields, an infallible field-forwarding
   constructor, independent setters, public mutable access, or unvalidated
   deserialization.

A public field alone is not a finding. A method named `validate` alone is not a
finding.

### Failing example: public field

```rust
pub struct Percentage {
    pub value: u8,
}

impl Percentage {
    pub fn validate(&self) -> Result<(), PercentageError> {
        if self.value <= 100 {
            Ok(())
        } else {
            Err(PercentageError(self.value))
        }
    }
}
```

Downstream code can construct `Percentage { value: 255 }`, so the nominal type
does not carry the advertised invariant.

Preferred representation:

```rust
pub struct Percentage(u8);

impl TryFrom<u8> for Percentage {
    type Error = PercentageError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        (value <= 100)
            .then_some(Self(value))
            .ok_or(PercentageError(value))
    }
}

impl Percentage {
    pub fn get(self) -> u8 {
        self.0
    }
}
```

### Failing example: independent setters

```rust
pub struct DateRange {
    start: Date,
    end: Date,
}

impl DateRange {
    pub fn set_start(&mut self, start: Date) {
        self.start = start;
    }

    pub fn set_end(&mut self, end: Date) {
        self.end = end;
    }

    pub fn validate(&self) -> Result<(), RangeError> {
        (self.start <= self.end)
            .then_some(())
            .ok_or(RangeError)
    }
}
```

The setters can independently create `start > end`. Better APIs include an
atomic `set_range(DateRange)`, validated replacement, or immutable construction.
A typestate design is not required for a simple value invariant.

### Invariant sources

The initial rule should accept invariant evidence from:

- a `TryFrom<Raw>` implementation that constructs the type only after a
  recognized intrinsic predicate;
- a fallible associated constructor returning `Result<Self, E>` or
  `Option<Self>`;
- a pure `validate(&self) -> Result<(), E>` or `is_valid(&self) -> bool` summary
  under the conservative rules from RFC 0003;
- assertions over persistent fields in every private constructor, when a public
  bypass exists; and
- a custom `Deserialize` or `serde(try_from = "Raw")` path that delegates to a
  fallible constructor.

Names can help locate candidates but cannot establish an invariant.

### Bypass sources

The initial construction inventory should include:

- public named fields;
- public tuple fields;
- `pub fn new(...) -> Self` bodies that forward unchecked parameters directly
  into invariant-bearing fields;
- `From<Raw> for Domain` implementations where conversion can construct
  rejected values;
- `Default` implementations and derives;
- public setters that mutate one participant in a cross-field invariant;
- `DerefMut`, `AsMut<Inner>`, or public `&mut` access to the underlying weak
  representation; and
- derived `Deserialize` when input fields can directly construct the domain
  type without a validating hook.

A bypass path must be reachable at the type's effective visibility. A private
unchecked constructor used only beneath one validated boundary does not widen
the downstream construction surface.

### Builder exception

Partial state is normal inside a builder:

```rust
struct RequestBuilder {
    method: Option<Method>,
    url: Option<Url>,
}

impl RequestBuilder {
    fn build(self) -> Result<Request, BuildError> {
        Ok(Request {
            method: self.method.ok_or(BuildError::MissingMethod)?,
            url: self.url.ok_or(BuildError::MissingUrl)?,
        })
    }
}
```

The builder is not a fragile `Request` if:

- it has no domain operations that assume completion;
- its broad state remains confined to assembly;
- `build` returns a distinct strong type; and
- the strong type does not expose equivalent bypasses.

The detector should classify shape and usage, not rely only on a `Builder`
suffix.

### Raw type exception

A raw type can intentionally expose fields when it converts once into a strong
type:

```rust
#[derive(serde::Deserialize)]
struct RawPercentage {
    value: u8,
}

impl TryFrom<RawPercentage> for Percentage {
    type Error = PercentageError;

    fn try_from(raw: RawPercentage) -> Result<Self, Self::Error> {
        Percentage::try_from(raw.value)
    }
}
```

`RawPercentage` is a representation type, not the domain value. The lint should
suppress it when local use confirms that it is consumed by fallible conversion
and not passed into domain operations as already valid.

### `bypassable_type_invariant` diagnostic

```plaintext
warning[DOMAIN104]: `Percentage` exposes construction paths that bypass its intrinsic invariant
  label: this validator rejects values above 100
  label: this public field can construct those values directly
  help: make invariant-bearing fields private and expose fallible construction
```

For cross-field setters:

```plaintext
help: replace independent setters with one validated update operation, or make the value immutable
```

No machine-applicable edit should change visibility or remove public methods.

## Proposed report: `invalid_default`

### `invalid_default` metadata

- **Rule code:** `DOMAIN105`
- **Kind:** Domain modelling
- **Initial level:** Report-only or `allow`
- **Analysis:** Constant/default construction extraction plus shared intrinsic
  predicate evaluation
- **Promotion target:** Experimental only for mechanically proven cases

### `invalid_default` rule statement

Report when `Default::default()` constructs a value that the same type's proven
intrinsic invariant rejects without external input or mutable state.

### `invalid_default` failing example

```rust
struct Endpoint {
    host: String,
    port: u16,
}

impl Default for Endpoint {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 0,
        }
    }
}

impl Endpoint {
    fn validate(&self) -> Result<(), EndpointError> {
        if self.host.is_empty() {
            return Err(EndpointError::EmptyHost);
        }
        if self.port == 0 {
            return Err(EndpointError::ZeroPort);
        }
        Ok(())
    }
}
```

The type's default is not merely incomplete for one operation. It violates both
intrinsic conditions advertised by its own validator.

### Proof boundary

The first implementation should report only when:

- the default body or derive can be reduced to direct field constants and known
  standard defaults;
- the invariant summary uses supported predicates such as non-zero, range,
  non-empty, option presence, or field correlation;
- evaluating the default against the summary has one deterministic result; and
- no external state or method with unknown effects participates.

Unsupported predicates should suppress the finding rather than guess.

### Derived `Default`

A derived default can be checked from each field's known `Default` where the
compiler can resolve a concrete value or symbolic property. Examples include:

- integers default to zero;
- booleans default to false;
- `Option<T>` defaults to `None`;
- standard collections and strings default to empty; and
- local field types can contribute a summarized default property.

The evaluator need not execute arbitrary user code. A symbolic lattice is
enough for the initial predicates:

```rust
pub enum DefaultFact {
    Zero,
    False,
    None,
    Empty,
    Constant(String),
    Struct(BTreeMap<String, DefaultFact>),
    Unknown,
}
```

Any `Unknown` needed by the invariant should suppress the report.

### Relationship with `new_without_default`

Clippy's `new_without_default` asks whether a zero-argument `new` conventionally
implies a useful default constructor. `DOMAIN105` asks whether the actual
default contradicts an intrinsic invariant. Both can be correct:

- a valid zero-argument `new` may deserve `Default`;
- a type with no valid neutral value should not invent one merely to satisfy a
  convention; and
- a builder or `Option<Domain>` may be a better representation for absence.

Whitaker should document this distinction and never recommend adding `Default`
when no valid default state exists.

### `invalid_default` diagnostic

```plaintext
advisory[DOMAIN105]: `Endpoint::default()` is rejected by `Endpoint`'s intrinsic invariant
  note: the default host is empty and the default port is zero
  help: remove `Default`, choose a valid neutral value, or move partial construction into a separate builder type
```

No automatic edit is safe.

## Shared state-space analysis

### Module layout

RFC 0003 introduces `common::domain_model`. This RFC adds:

```text
common/src/domain_model/state_space/
├── mod.rs
├── fields.rs
├── constraints.rs
├── construction.rs
├── evidence.rs
├── evaluation.rs
└── diagnostics.rs
```

Lint crates remain separate:

```text
crates/
├── manual_tagged_union/
├── correlated_optional_fields/
├── mutually_exclusive_bool_fields/
├── bypassable_type_invariant/
└── invalid_default/
```

The first implementation may share one crate-post type collector through
`common`, but each lint must own its declaration, configuration, diagnostics,
and tests.

### Field roles

The pure model should classify fields into roles without encoding rustc types:

```rust
pub enum FieldRole {
    Discriminator,
    OptionalPayload,
    BooleanFlag,
    Scalar,
    Collection,
    NestedDomain,
    Unknown,
}
```

A field can carry several candidate roles during collection. Evaluation chooses
roles only when surrounding evidence disambiguates them.

### Constraints

The initial constraint language should remain finite and inspectable:

```rust
pub enum StateConstraint {
    ExactlyOne(Vec<FieldKey>),
    AtMostOne(Vec<FieldKey>),
    AllOrNone(Vec<FieldKey>),
    Implies {
        premise: FieldPredicate,
        consequence: FieldPredicate,
    },
    MutuallyExclusive(Vec<FieldPredicate>),
    TagSelectsPayload {
        tag: FieldKey,
        cases: Vec<TagPayloadCase>,
    },
    OrderedPair {
        lower: FieldKey,
        upper: FieldKey,
        inclusive: bool,
    },
    ScalarRange {
        field: FieldKey,
        lower: Option<i128>,
        upper: Option<i128>,
    },
    NonEmpty(FieldKey),
}
```

`OrderedPair`, `ScalarRange`, and `NonEmpty` primarily support
`bypassable_type_invariant` and `invalid_default`. The sum-type rules can begin
with presence and boolean constraints.

### Evidence strength

Each inferred constraint should carry:

```rust
pub enum EvidenceStrength {
    Exhaustive,
    Repeated,
    ConstructorAndConsumer,
    Partial,
}
```

The source records should identify:

- the body and span;
- accepted and rejected state rows;
- whether rejection returns an error, panics, or marks unreachable;
- whether the code is a constructor, validator, consumer, or debug assertion;
- whether the body is generated or test-only; and
- whether external state participates.

Only `Exhaustive`, `Repeated`, and selected `ConstructorAndConsumer` evidence
should trigger initial diagnostics. `Partial` evidence can feed debug output and
future corpus analysis.

### Type-level collection

A crate-wide late pass should build one `TypeStateSummary` per local nominal
type:

```rust
pub struct TypeStateSummary {
    pub type_name: String,
    pub effective_visibility: VisibilityClass,
    pub fields: Vec<FieldSummary>,
    pub constraints: Vec<ConstraintEvidence>,
    pub construction_paths: Vec<ConstructionPath>,
    pub operation_roles: Vec<OperationRole>,
    pub representation_markers: RepresentationMarkers,
}
```

Collection should include inherent methods and local trait implementations for
the type. Consumers outside an `impl` can contribute correlation evidence when
their parameter type resolves to the nominal type.

Cross-crate consumers remain out of scope in the first release. Public API
findings should therefore use more conservative thresholds because local code
may not reveal every legitimate state.

### HIR and MIR responsibilities

HIR is sufficient for:

- field and visibility inventory;
- match and guard extraction;
- resolved constructor and trait paths;
- direct public setter bodies;
- `Default`, `From`, `TryFrom`, and `Deserialize` implementation discovery; and
- macro and source-span filtering.

MIR confirmation is useful for:

- field mutation through projections and aliases;
- constructor forwarding hidden by temporaries;
- `DerefMut` and `AsMut` exposure;
- ensuring an invalid arm is reachable from safe construction paths; and
- proving that a raw or builder type does not escape into domain operations.

The first two sum-type rules should not wait for a full MIR implementation when
an exhaustive HIR match already supplies decisive evidence.

## Diagnostic precedence and deduplication

Several rules can describe the same type. Whitaker should emit one primary
finding per correlated field group, selected by specificity:

1. `manual_tagged_union`;
2. `correlated_optional_fields`;
3. `mutually_exclusive_bool_fields`;
4. `bypassable_type_invariant`; and
5. `invalid_default` as a secondary advisory when it adds distinct evidence.

`bypassable_type_invariant` may still report a separate scalar invariant on the
same type when it involves different fields from a tagged-union group.

Diagnostics should carry a stable group fingerprint built from type identity,
field identities, and normalized constraint. This supports deduplication in
SARIF and future `whitaker check` aggregation.

## Exemption model

### Builders and partial assembly

Partial products are legitimate when they are visibly staged and convert into a
distinct strong type. Suppression should rely on behaviour:

- the type's methods predominantly set or accumulate fields;
- one terminal method consumes `self` and returns a distinct type;
- domain operations accept the built type, not the builder; and
- the partial type does not claim the strong type's invariant.

A type name ending in `Builder` is weak supporting evidence only.

### Raw, transport, and serialization types

Derived `Deserialize` into a raw model is expected. Derived `Deserialize`
directly into an invariant-bearing public domain type can be bypass evidence.

Strong suppression evidence includes:

- `#[serde(try_from = "RawType")]`;
- a custom `Deserialize` implementation that delegates to a fallible
  constructor;
- local use confined to a fallible `TryFrom<Raw>` conversion; and
- explicit configuration marking a module as a representation boundary.

`#[serde(default)]` deserves particular care because it can create field
combinations absent from the input. It is not invalid by itself; it simply
widens the construction surface considered by `DOMAIN104` and `DOMAIN105`.

### Foreign layouts

Types with `#[repr(C)]`, `#[repr(transparent)]`, bindgen markers, or configured
FFI modules should pass by default when their layout mirrors an external
contract. A separate strong wrapper should remain the recommended inward
boundary.

A transparent newtype can itself be the strong type, so `repr(transparent)`
should suppress layout-changing suggestions but not a proven public-field
bypass where the wrapper claims an invariant.

### Protocol decoders and transactional mutation

Decoders often inhabit partial states while consuming input. A private decoder
state with no domain operations and one terminal `finish() -> Result<Strong, E>`
should pass.

Similarly, a private transactional mutation scope may temporarily violate a
cross-field invariant before commit or rollback. The lint should suppress when
all invalid intermediate states remain unobservable and the transaction cannot
escape. Public independent setters do not receive this exemption.

### Runtime resource state

Sockets, tasks, files, database transactions, and actor handles can change state
because of external events. Enums may still improve their APIs, but generic
value-invariant analysis should not claim that a wrapper permanently proves a
live resource state.

The sum-type rules can diagnose a purely internal runtime state representation
when correlation evidence is clear. `bypassable_type_invariant` should suppress
predicates involving I/O, clocks, synchronization, or external handles.

### External trait and framework contracts

A type may expose fields or setters to satisfy an external trait, derive macro,
or framework reflection contract. Resolved external obligations and generated
code should lower confidence or suppress the finding. Configuration can mark
specific traits and modules as imposed representation boundaries.

## Configuration

The family should reuse shared exclusion and resolved-path parsing from RFC
0003. Proposed rule-specific keys are:

```toml
[lint]
experimental = true
extend-select = ["DOMAIN"]

[lint.manual_tagged_union]
minimum-evidence-sites = 1
excluded-paths = [
  "crate::wire::raw",
]

[lint.correlated_optional_fields]
minimum-evidence-sites = 2

[lint.mutually_exclusive_bool_fields]
clippy-first = true

[lint.bypassable_type_invariant]
representation-types = [
  "crate::api::RawRequest",
]
builder-terminal-methods = ["build", "finish", "try_build"]
excluded-traits = [
  "external_framework::Reflect",
]

[lint.invalid_default]
report-symbolic-defaults = true
```

Names such as `build` and `finish` must not suppress alone. They identify
candidate terminal methods whose return type and use pattern still require
verification.

The default `minimum-evidence-sites` for `manual_tagged_union` can be one only
when that site is exhaustive and rejects all mismatched rows. Partial sites
must obey the higher shared threshold.

## Diagnostics and localization

Each rule should provide primary, labels, note, and help messages through
Fluent. Initial slugs include:

```plaintext
manual_tagged_union.primary
manual_tagged_union.discriminator_label
manual_tagged_union.payload_label
manual_tagged_union.evidence_label
manual_tagged_union.note
manual_tagged_union.help
correlated_optional_fields.primary
correlated_optional_fields.field_label
correlated_optional_fields.evidence_label
correlated_optional_fields.note
correlated_optional_fields.help
mutually_exclusive_bool_fields.primary
mutually_exclusive_bool_fields.field_label
mutually_exclusive_bool_fields.note
mutually_exclusive_bool_fields.help
bypassable_type_invariant.primary
bypassable_type_invariant.invariant_label
bypassable_type_invariant.bypass_label
bypassable_type_invariant.note
bypassable_type_invariant.help
invalid_default.primary
invalid_default.default_label
invalid_default.invariant_label
invalid_default.note
invalid_default.help
```

Structured arguments should include type name, field names, constraint kind,
accepted state count, representable state count where bounded, construction
path kind, and evidence strength.

Messages should avoid “invalid states are representable” when the detector has
only partial evidence. That phrase is appropriate only when the reported
constraint and bypass are mechanically established.

English (`en-GB`), Welsh (`cy`), and Gaelic (`gd`) resources should ship with
each implemented lint.

## Testing requirements

### Pure constraint tests

Unit and property tests should cover:

- all truth tables for two, three, and bounded four-field presence groups;
- exactly-one, at-most-one, all-or-none, implication, and mutual-exclusion
  recognition;
- permutation invariance of field order;
- deterministic normalization and fingerprints;
- contradictory evidence from different consumers;
- evidence-strength promotion rules;
- deduplication precedence among overlapping lints; and
- symbolic defaults for zero, false, none, empty, constants, and unknowns.

A bounded exhaustive test can enumerate every boolean truth table up to four
fields and prove that the classifier recognizes only the intended canonical
constraints.

### Behaviour-driven tests

`rstest-bdd` scenarios should include:

- an enum tag selecting optional payloads;
- common metadata surrounding a tagged payload group;
- exactly one of two credential fields;
- independent search options that must pass;
- two- and three-boolean one-hot states;
- unrelated booleans that must pass;
- a public field bypassing a scalar range invariant;
- independent setters bypassing an ordered-pair invariant;
- a builder converting into a distinct strong type;
- a raw serde model converting through `TryFrom`;
- derived deserialization directly into a fragile domain type;
- a valid default;
- a directly provable invalid default; and
- an unknown default or predicate that must suppress.

### UI tests

UI fixtures should cover:

- enum, bool, integer, and string discriminators;
- exhaustive and partial matches;
- `unreachable!`, panic, `Err`, and debug-assert enforcement;
- public and private field visibility;
- tuple structs;
- `Default`, `From`, `TryFrom`, `DerefMut`, and `AsMut` surfaces;
- `serde(try_from)`, custom `Deserialize`, and derived `Deserialize`;
- `repr(C)` and transparent wrappers;
- macro-generated fields and matches;
- external trait implementations;
- Clippy running beside `mutually_exclusive_bool_fields`; and
- localized output.

### Mutation and formal checks

Property tests should validate the pure truth-table evaluator. Kani can bound
construction-surface combinations for the symbolic default and constraint
lattices if ordinary exhaustive tests become unwieldy. Verus is unnecessary
unless the shared normalization algorithm acquires a non-trivial lemma not
already covered by bounded enumeration.

### Downstream corpus

The experimental suite should run against:

- configuration-heavy crates with many independent options;
- protocol and serialization crates with raw models;
- state-machine-heavy asynchronous code;
- domain-model crates using newtypes and enums well; and
- at least one framework-driven crate with reflection or generated data models.

Findings should be classified by rule and exception model. Promotion should
require evidence that builders, raw types, runtime resources, and independent
option sets remain quiet.

## Compatibility and migration

All rules are additive and experimental. No public API changes occur merely by
upgrading Whitaker unless the family is explicitly selected.

Migration can be source-breaking when a public struct becomes an enum or gains
private fields. Diagnostics should therefore prioritize private and crate-local
types during early adoption. Public findings remain useful, but their help text
should mention compatibility adapters.

A public compatibility layer can preserve the old representation while moving
the internal domain to a strong type:

```rust
#[derive(serde::Deserialize)]
pub struct MessageDto {
    kind: MessageKind,
    text: Option<String>,
    bytes: Option<Vec<u8>>,
}

impl TryFrom<MessageDto> for Message {
    type Error = MessageError;

    fn try_from(dto: MessageDto) -> Result<Self, Self::Error> {
        match (dto.kind, dto.text, dto.bytes) {
            (MessageKind::Text, Some(text), None) => Ok(Self::Text(text)),
            (MessageKind::Binary, None, Some(bytes)) => Ok(Self::Binary(bytes)),
            _ => Err(MessageError::InvalidPayloadCombination),
        }
    }
}
```

This architecture makes the invalid product explicit at the transport boundary
and keeps it out of domain operations.

A typical migration sequence is:

1. identify the strongest correlation and add a private enum or newtype;
2. convert existing constructors through the strong type;
3. move internal consumers to the strong representation;
4. preserve a DTO or adapter for public and serialized compatibility;
5. make invariant-bearing fields private;
6. replace independent mutators with atomic validated updates; and
7. remove obsolete validators and unreachable arms after all paths migrate.

Suppressions should identify deliberate representation types rather than hide
individual evidence matches throughout the crate.

## Alternatives considered

### One `fragile_type` lint

A single lint could collect every state-space concern and choose a message. This
would make adoption and suppression coarse, and it would obscure materially
different evidence thresholds. A manual tagged union can be highly certain from
one exhaustive match; an invalid default needs a separate symbolic proof.
Distinct rules allow independent maturation.

### Rely on field counts

Counting options or booleans is cheap but semantically weak. Clippy already owns
the excessive-boolean count heuristic. Whitaker's value lies in showing that
specific combinations are rejected or treated as impossible.

### Diagnose any `validate(&self)` method

Validation methods can express contextual operation preconditions, remote
checks, or optional stricter modes. The proposal requires a summarized intrinsic
predicate and an exposed bypass path. Names alone are insufficient.

### Require private fields for every domain type

Public fields are reasonable for plain data and raw models. Privacy matters only
when construction must preserve an invariant. The lint therefore correlates
field visibility with actual invariant evidence.

### Prefer typestate universally

Typestate can encode lifecycle transitions, but it adds generic parameters,
state marker types, and API surface. Enums and validated immutable values are
simpler for many cases. Diagnostics should recommend the least elaborate type
shape supported by the evidence.

### Use runtime assertions instead

Assertions can detect corruption but do not narrow the construction surface.
They also convert a modelling issue into a panic path. Assertions remain useful
for unsafe boundaries and internal compiler-bug checks, but they do not replace
representational constraints.

### Treat serde validation as sufficient

A validated deserializer protects one ingress path. Public fields, constructors,
and mutation methods may still bypass the invariant. Conversely, a raw derived
serde type followed by `TryFrom` is a sound boundary and should pass. The lint
must inventory the whole local construction surface.

### Implement `invalid_default` immediately as a warning

Arbitrary `Default` and validator code can be complex. Premature warning status
would either overclaim or require executing user code in the compiler. A small
symbolic subset should begin report-only and expand only with proof-preserving
semantics.

## Open questions

- Should one exhaustive consumer match suffice for `DOMAIN102`, or should
  correlated options always require a constructor or second consumer as
  corroboration?
- How should contradictory local consumers affect a finding: suppress entirely,
  or report that the type's state contract is inconsistent?
- Should `manual_tagged_union` recognize collection emptiness as payload absence
  in its first release?
- Can local enum discriminators be considered closed when marked
  `#[non_exhaustive]`?
- Which serde attributes should count as validated construction beyond
  `try_from` and a custom `Deserialize` body?
- Should `DerefMut` exposure always count as a bypass, or only when the target
  type maps directly onto invariant-bearing fields?
- How should public types be scored when local code supplies strong evidence but
  external consumers may rely on additional legal states?
- Should `DOMAIN105` remain a CLI report until symbolic default summaries can be
  serialized and explained independently of the compiler diagnostic?
- Is `DOMAIN103` valuable after a real `struct_excessive_bools` trial, or should
  boolean-only states fold into `manual_tagged_union`?

## Recommendation

Accept `DOMAIN101` through `DOMAIN105` as the fragile-type subfamily, but
implement them in confidence order.

Begin with `manual_tagged_union`. Its exhaustive-match form offers the clearest
mapping from representable product to legal sum and the most actionable enum
remediation. Add `correlated_optional_fields` next with a two-site threshold
except for complete truth-table matches.

Run Clippy's `struct_excessive_bools` across the target corpus before deciding
whether `mutually_exclusive_bool_fields` has enough residual value to implement.
Build `bypassable_type_invariant` only after the shared intrinsic-predicate and
construction-surface models can distinguish domain types from builders and raw
representations. Keep `invalid_default` report-only until its symbolic evaluator
can explain every fact used in the contradiction.

This order starts with field correlations that code already states loudly, then
moves towards broader API reasoning as the evidence machinery matures. The
result should feel less like a style tribunal and more like a cartographer
pointing out where the type system's map stops before the territory does.

## References

[^clippy-struct-excessive-bools]: [Clippy `struct_excessive_bools` documentation](https://rust-lang.github.io/rust-clippy/master/index.html#struct_excessive_bools)

[^clippy-fn-params-excessive-bools]: [Clippy `fn_params_excessive_bools` documentation](https://rust-lang.github.io/rust-clippy/master/index.html#fn_params_excessive_bools)

[^clippy-unnecessary-unwrap]: [Clippy `unnecessary_unwrap` documentation](https://rust-lang.github.io/rust-clippy/master/index.html#unnecessary_unwrap)

[^clippy-new-without-default]: [Clippy `new_without_default` documentation](https://rust-lang.github.io/rust-clippy/master/index.html#new_without_default)
