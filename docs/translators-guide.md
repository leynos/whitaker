# Whitaker translator's guide

This guide explains how Whitaker's diagnostics are localized: where the
catalogues live, how a locale is selected, what each message receives at
runtime, and how to add or update a translation and prove that it works. It
owns the list of shipped locales, the fallback policy, and the technical gates.
Wording decisions belong elsewhere: the
[localization style guide](localization-styleguide.md) owns voice, tone, and
writing mechanics, and the [localization glossary](localization-glossary.md)
owns terminology and each locale's register.

## 1. Introduction

Whitaker uses [Project Fluent](https://projectfluent.org/) for localization,
through the `fluent-templates` crate. The catalogues are embedded in the
`whitaker-common` crate at compile time, so no lint reads a translation file at
runtime.

The localized surface is the diagnostics the lints emit: each lint's primary
message, note, help, and, where present, span label and suggestion label. The
`whitaker` command-line interface, the installer, and log output are not
localized.

`en-GB` is the source locale. It defines the files, messages, and attributes
every other catalogue must match, and it renders any message a translation has
not yet covered.

A lint chooses its locale once, when the lint pass is constructed, from these
candidates in order:

1. The `DYLINT_LOCALE` environment variable.
2. The `locale` key in `dylint.toml`.
3. The `en-GB` fallback.

`get_localizer_for_lint` in `common/src/i18n/helpers.rs` reads the candidates
and `resolve_localizer` in `common/src/i18n/selection.rs` applies the order. A
candidate that is empty, or that names a locale Whitaker does not ship, is
skipped with a warning in the log and the next candidate is tried. The
[user's guide](users-guide.md#localized-diagnostics) describes the settings
from the user's side.

### Shipped locales

Whitaker ships 35 locales. This table is the documentation's single list of
them; other documents link here instead of repeating it.

| Script family         | Tags                                                                                                                                                        |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Latin                 | `cs`, `cy`, `da`, `de`, `en-GB`, `en-US`, `es-419`, `es-ES`, `fi`, `fr`, `gd`, `hu`, `id`, `it`, `nb`, `nl`, `pl`, `pt-BR`, `pt-PT`, `ro`, `sv`, `tr`, `vi` |
| Cyrillic              | `ru`, `uk`                                                                                                                                                  |
| Greek                 | `el`                                                                                                                                                        |
| Right-to-left         | `ar`, `fa`, `he`                                                                                                                                            |
| Devanagari            | `hi`                                                                                                                                                        |
| Thai                  | `th`                                                                                                                                                        |
| Han, Kana, and Hangul | `ja`, `ko`, `zh-Hans`, `zh-Hant`                                                                                                                            |

_Table 1: Locales Whitaker ships, by script family._

Shipping a catalogue is not the same as having reviewed it. `en-GB`, `cy`, and
`gd` predate this guide. `en-US` is a spelling variant of the source. The
remaining 31 catalogues were drafted by an AI model and have not been reviewed
by a fluent reader; the [glossary](localization-glossary.md#locale-terminology)
records that status and how to lift it.

## 2. The locale registry

The directory tree is the registry. `static_loader!` in
`common/src/i18n/mod.rs` embeds every directory under `common/locales/` as one
locale, named by the directory, and `available_locales` in
`common/src/i18n/locales.rs` reports whatever the loader embedded. There is no
separate list of tags in production code to keep in step.

`common/build.rs` registers `common/locales/` as a build input, so adding a
directory or a file rebuilds the embedded bundle. Without it, a new file would
be invisible to an already-built crate.

One list is deliberately independent of the registry: `EXPECTED_TAGS` in
`common/tests/i18n_catalogue_parity/rules.rs`. A test that only read the loader
back could never notice a locale added or dropped by accident, so the parity
tests compare both the loader and the directory tree against that written-out
list.

### Fallback policy

Two different mechanisms are both called fallback, and they behave differently.

**Locale selection** matches whole tags. `supports_locale` parses the requested
tag, normalizes its case, and accepts it only if a directory with exactly that
tag exists. There is no matching by language, region, or script:

| Requested tag          | Outcome                                       |
| ---------------------- | --------------------------------------------- |
| `pt-BR`, `PT-br`       | `pt-BR`                                       |
| `zh-Hans`, `zh-hans`   | `zh-Hans`                                     |
| `pt`, `es`, `zh`, `en` | unsupported; the next candidate, then `en-GB` |
| `zh-CN`, `fr-CA`       | unsupported; the next candidate, then `en-GB` |
| `no`                   | unsupported; `nb` must be requested by name   |

_Table 2: How requested tags resolve._

A user must therefore request a shipped tag exactly. This is observed
behaviour, exercised by `locale_selection_matches_whole_tags` in the parity
tests, not a design goal: a request for `es` or `zh-CN` falling through to
English is a known limitation, and a negotiation rule for bare languages and
unlisted regions is proposed follow-up work. Until it exists, do not document a
bare tag such as `es` as supported.

**Message lookup** falls back per message. When the selected locale lacks a
message or an attribute, the loader renders the `en-GB` text for that one
string. Scottish Gaelic has no `common-lint-count.fallback-note`, and the test
`untranslated_keys_fall_back_to_the_source_text` shows the English text
appearing in its place. A catalogue that is missing a string therefore still
works, but renders mixed-language output, so the parity tests treat a missing
key as a failure unless it is listed as a known gap.

If a message is missing from every catalogue, including `en-GB`, the lint logs
the failure, reports a delayed compiler bug, and emits English wording that is
built into the lint's Rust source.

## 3. File structure

Each locale is one directory under `common/locales/`, holding one Fluent file
per lint plus `common.ftl` for shared helpers:

```plaintext
common/locales/
├── en-GB/
│   ├── bumpy_road_function.ftl
│   ├── common.ftl
│   ├── conditional_max_n_branches.ftl
│   └── …
├── cy/
│   └── …
└── zh-Hant/
    └── …
```

Every locale directory holds the same 13 file names as `en-GB`. The loader
merges all the files of a directory into one bundle, so a message identifier
must be unique across the whole locale, not just within its file. Nothing is
generated: translators edit the `.ftl` files directly.

### Fluent file format

A Fluent Translation List (FTL) file holds messages, each with an optional
value and optional attributes:

```ftl
## Module length guidance.

# A comment for translators.
module_max_lines = Module { $module } spans { $lines } lines, exceeding the allowed { $limit }.
    .note = Large modules are harder to navigate and review.
    .help = Split { $module } into smaller modules or reduce its responsibilities.
```

The rules that matter in this repository are:

- A line beginning with an identifier and `=` starts a message. Indented lines
  beginning with `.` are its attributes.
- `{ $module }` is a placeable that interpolates a variable.
- A line beginning with `-` defines a term. `common.ftl` defines
  `-brand-name-whitaker` and four `-term-*` terms; no message references them
  at present, but every locale carries them.
- A comment is `#`, `##`, or `###` followed by a space. Any other marker, such
  as `#.`, is a syntax error. Translate the `##` heading comment of each file;
  keep `#` comments for translators in English.
- A pattern continues on the next line only if that line is indented. Keep
  each pattern on one line, except for select expressions.

A syntax error does not break the build. The loader keeps the entries it could
parse and silently discards the rest, which is why the parity tests parse every
file strictly.

## 4. Message key conventions

A lint's message identifier is the lint's name, in snake case, and its file has
the same name. The identifier is also a Fluent identifier without further
processing. The shared helpers in `common.ftl` use a `common-` prefix with
hyphens.

| Message                                  | Attributes                                | Emitted by                                |
| ---------------------------------------- | ----------------------------------------- | ----------------------------------------- |
| `bumpy_road_function`                    | `.note`, `.help`, `.label`                | `crates/bumpy_road_function`              |
| `conditional_max_n_branches`             | `.note`, `.help`                          | `crates/conditional_max_n_branches`       |
| `function_attrs_follow_docs`             | `.note`, `.help`                          | `crates/function_attrs_follow_docs`       |
| `macro_aware_cognitive_complexity`       | `.note`, `.help`                          | `crates/macro_aware_cognitive_complexity` |
| `module_max_lines`                       | `.note`, `.help`                          | `crates/module_max_lines`                 |
| `module_must_have_inner_docs`            | `.note`, `.help`                          | `crates/module_must_have_inner_docs`      |
| `no_expect_outside_tests`                | `.note`, `.help`                          | `crates/no_expect_outside_tests`          |
| `no_std_fs_operations`                   | `.note`, `.help`                          | `crates/no_std_fs_operations`             |
| `no_std_fs_operations_unknown_keys`      | `.note`, `.help`                          | `crates/no_std_fs_operations`             |
| `no_std_fs_operations_discarded_section` | `.note`, `.help`                          | `crates/no_std_fs_operations`             |
| `no_unwrap_or_else_panic`                | `.note`, `.help`                          | `crates/no_unwrap_or_else_panic`          |
| `public_fn_must_have_docs`               | `.note`, `.help`                          | no lint at present                        |
| `string_continuation_style`              | `.note`, `.help`, `.label`, `.suggestion` | `crates/string_continuation_style`        |
| `test_must_not_have_example`             | `.note`, `.help`                          | `crates/test_must_not_have_example`       |
| `common-lint-count`                      | `.note`, `.help`, `.fallback-note`        | tests only                                |
| `common-attribute-fallback`              | none                                      | `crates/function_attrs_follow_docs`       |

_Table 3: Messages, their attributes, and the code that emits them._

Every lint resolves the value, `.note`, and `.help` together through
`resolve_message_set` in `common/src/i18n/diagnostics.rs`, so all three must
exist. Translate the messages that no lint emits at present as carefully as the
rest: they are part of the catalogue contract, and the parity tests require
them.

Do not rename an identifier or an attribute. The identifier is how the lint
finds its text.

## 5. Variable usage

A placeable such as `{ $module }` is replaced at runtime by a value the lint
supplies. A translation may move a placeable anywhere in the sentence and may
repeat it, but it may use only the variables its message receives.

### Variables by message

| Message                                  | Variable         | Type             | Content                                                          |
| ---------------------------------------- | ---------------- | ---------------- | ---------------------------------------------------------------- |
| `bumpy_road_function`                    | `$name`          | string           | Function name; code                                              |
|                                          | `$count`         | number           | Number of bumps                                                  |
|                                          | `$threshold`     | number           | Configured threshold; may be fractional                          |
|                                          | `$index`         | number           | Position of a bump, in `.label`                                  |
|                                          | `$lines`         | number           | Lines a bump spans, in `.label`                                  |
| `conditional_max_n_branches`             | `$name`          | string           | English phrase: "if condition", "while condition", "match guard" |
|                                          | `$branches`      | number           | Branches found                                                   |
|                                          | `$limit`         | number           | Configured maximum                                               |
|                                          | `$branch_phrase` | string           | Count and noun, rendered in Rust; see section 6                  |
|                                          | `$limit_phrase`  | string           | Limit and noun, rendered in Rust; see section 6                  |
| `function_attrs_follow_docs`             | `$subject`       | string           | English phrase: "functions", "methods", "trait methods"          |
|                                          | `$attribute`     | string           | Source text of the attribute; code                               |
| `macro_aware_cognitive_complexity`       | `$name`          | string           | Function name; code                                              |
|                                          | `$score`         | number           | Measured complexity                                              |
|                                          | `$limit`         | number           | Configured maximum                                               |
| `module_max_lines`                       | `$module`        | string           | Module name; code                                                |
|                                          | `$lines`         | number           | Lines in the module                                              |
|                                          | `$limit`         | number           | Configured maximum                                               |
| `module_must_have_inner_docs`            | `$module`        | string           | Module name; code                                                |
| `no_expect_outside_tests`                | `$receiver`      | string           | Type of the receiver, already wrapped in backticks               |
|                                          | `$context`       | string           | English phrase with code, such as "function `helper`"            |
|                                          | `$handling`      | string           | Selector: `option`, `result`, or `other`                         |
| `no_std_fs_operations`                   | `$operation`     | string           | Path of the operation; code                                      |
| `no_std_fs_operations_unknown_keys`      | `$keys`          | string           | Bounded, quoted list of the ignored keys; code                   |
|                                          | `$version`       | string           | Package version of the suite that loaded the configuration       |
|                                          | `$revision`      | string           | Suite revision, or `unknown` when the build could not probe it   |
| `no_std_fs_operations_discarded_section` | `$error`         | string           | Bounded parse error that discarded the section                   |
|                                          | `$version`       | string           | Package version of the suite that loaded the configuration       |
|                                          | `$revision`      | string           | Suite revision, or `unknown` when the build could not probe it   |
| `no_unwrap_or_else_panic`                | `$receiver`      | string           | Type of the receiver, already wrapped in backticks               |
| `public_fn_must_have_docs`               | `$function`      | string           | Function name; code                                              |
| `test_must_not_have_example`             | `$test`          | string           | Test function name; code                                         |
|                                          | `$reason`        | string           | English phrase: "an examples heading", "a fenced code block"     |
| `common-lint-count`                      | `$lint`          | string or number | A lint name, or a count in the Welsh catalogue                   |

_Table 4: Variables each message receives._

### Rules for variables

- **Code values pass through untouched.** Function names, module names, types,
  and attribute text are opaque. Never inflect, translate, or re-quote them.
  `$receiver` brings its own backticks; do not add more.
- **Numbers are formatted by the program.** They render as plain ASCII digits
  with no grouping separator in every locale, as the test
  `numbers_render_without_locale_formatting` shows. A translation chooses the
  words around a number, not its digits.
- **English phrases cannot be translated in the catalogue.** `$name` in
  `conditional_max_n_branches`, `$subject`, `$reason`, and `$context` arrive as
  English text. Write the sentence so the phrase needs no inflection, article,
  or agreement, and enclose the placeable in the locale's quotation marks.
  Making these phrases translatable needs a change to the lints and is proposed
  follow-up work.
- **`$handling` is a selector, not text.** It chooses a variant in
  `no_expect_outside_tests.help`. Keep exactly the variants `[option]`,
  `[result]`, and `*[other]`.

The parity tests enforce the contract structurally. They walk the parsed Fluent
tree, so a variable that appears only inside one variant of a select still
counts. A translation fails if it uses a variable its message does not receive,
or if it drops one the source uses. The Welsh and Gaelic catalogues drop
`$name` and `$reason` in two notes to avoid embedding an English phrase; those
cases are listed by name in `KNOWN_DROPPED_VARIABLES`, and a new one must be
added there deliberately.

## 6. Plural forms

A select expression chooses a variant by the plural category of a number:

```ftl
    .label = Complexity bump { $index } spans { $lines } { $lines ->
        [one] line
       *[other] lines
    }.
```

The variant marked `*` is the default and is required. A variant key may also
be an exact number, such as `[0]`, which is matched before any category.

### Plural engine and categories

Fluent asks the `intl_pluralrules` crate for the category. The version in
`Cargo.lock` is 7.0.2, generated from Common Locale Data Repository (CLDR)
release 37, and it can differ from the current
[CLDR plural rules](https://cldr.unicode.org/index/cldr-spec/plural-rules).
These are the cardinal categories it selects for the shipped locales:

| Categories                                   | Locales                                                                                                                             |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `other`                                      | `id`, `ja`, `ko`, `th`, `vi`, `zh-Hans`, `zh-Hant`                                                                                  |
| `one`, `other`                               | `da`, `de`, `el`, `en-GB`, `en-US`, `es-419`, `es-ES`, `fa`, `fi`, `fr`, `hi`, `hu`, `it`, `nb`, `nl`, `pt-BR`, `pt-PT`, `sv`, `tr` |
| `one`, `few`, `other`                        | `ro`                                                                                                                                |
| `one`, `few`, `many`, `other`                | `cs`, `pl`, `ru`, `uk`                                                                                                              |
| `one`, `two`, `few`, `other`                 | `gd`                                                                                                                                |
| `one`, `two`, `many`, `other`                | `he`                                                                                                                                |
| `zero`, `one`, `two`, `few`, `many`, `other` | `ar`, `cy`                                                                                                                          |

_Table 5: Plural categories by shipped locale._

A select on `$count`, `$lines`, `$branches`, or `$limit` must list every
category in the locale's row, and no others. `check_plural_selects` in the
parity tests enforces this against the same table, so dropping Polish `few` or
Arabic `two` fails the suite instead of quietly losing a form. The table is
itself checked against the engine. For boundary counts and generated counts up
to 10,000, in every shipped locale, the property tests ask `intl_pluralrules`,
negotiated the way Fluent negotiates it, for each count's category. They fail
if the table omits that category, or if a probe message with one marker per
listed category, rendered through the loader's bundle type, selects a different
one. Two qualifications apply:

- Exact-number variants such as `[0]` may sit beside the categories, but they
  never replace one: `[0]` covers zero, not every value in a category. The Welsh
  `common-lint-count`, which spells out `[0]` to `[3]` with only an `*[other]`
  default, is the one listed exception, named in `KNOWN_ENUMERATED_SELECTS` in
  the same file.
- A locale whose only category is `other` needs no select at all.

`one` does not always mean exactly one: in French it also covers zero. Listing
a category does not imply different wording either. Hungarian and Turkish keep
the noun singular after a numeral, so both variants may read alike. Czech
`many` applies only to fractions, which no lint passes today, but the variant
is still required.

Prefer a variant that is a whole sentence wherever agreement spreads beyond one
noun, as the Welsh and Arabic catalogues do. Splicing a lone noun into a fixed
sentence works for English and few other languages.

### What reaches the plural resolver

Counts reach Fluent as numbers, so category selection works at runtime. The
lints pass each count as a signed 64-bit integer. The test
`plural_variants_follow_numeric_arguments` renders `bumpy_road_function.label`
for counts in different categories and requires different wording for each.
Only cardinal categories are used; no message selects on an ordinal.

### The branch phrase helper

`conditional_max_n_branches` is the exception to "the catalogue owns the
wording". The lint passes two extra strings, `$branch_phrase` and
`$limit_phrase`, which the Rust function `branch_phrase` in
`common/src/i18n/helpers.rs` renders as a count and noun. That function knows
only English, Welsh, and Scottish Gaelic, and renders English for every other
locale.

The source, Welsh, and Gaelic catalogues use the two phrases. Every other
catalogue must ignore them and select on the numbers `$branches` and `$limit`,
which the lint also passes:

```ftl
# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Réduisez « { $name } » à { $limit } branche au maximum.
       *[other] Réduisez « { $name } » à { $limit } branches au maximum.
    }
```

This is the value of the shipped French message. The parity tests reject
`$branch_phrase` or `$limit_phrase` in any locale the helper cannot serve. Do
not extend the Rust helper for a new locale.

## 7. Adding or updating a locale

A locale tag need not introduce a new language. It can be a region or script
variant of a language Whitaker already ships, because selection is by whole
tag. The steps below add Icelandic (`is`) as an example.

### Step 1: Start from the source catalogue

From the repository root:

```sh
mkdir common/locales/is
cp common/locales/en-GB/*.ftl common/locales/is/
```

The copy is a scaffold, not a deliverable. The parity tests reject a
non-English locale whose rendered text equals the source.

### Step 2: Agree the vocabulary

Read the [style guide](localization-styleguide.md) and the source tables and
localizability notes in the [glossary](localization-glossary.md). Add a section
for the locale to the glossary: its register notes and its four-column table
for the difficult terms. Decide the address form and the instruction form
before translating the first message.

### Step 3: Translate

Edit each file in `common/locales/is/`:

- Translate every value and attribute. Keep every identifier and attribute
  name.
- Use only the variables in Table 4, and follow the rules in section 5.
- Write plural selects with the categories in Table 5. If the language is not
  in the table, find its categories in the `intl_pluralrules` source and add a
  row.
- In `conditional_max_n_branches.ftl`, select on `$branches` and `$limit` as
  section 6 describes.
- Keep frozen tokens exactly as the glossary lists them.
- Translate the `##` comment at the top of each file.

### Step 4: Register the locale in the test oracle

The directory is the production registry, so no production code changes. Two
test tables in `common/tests/i18n_catalogue_parity/rules.rs` need the tag:

1. Add it to `EXPECTED_TAGS`, in sorted order, and update the array length.
2. If its plural categories are not `one` and `other`, add it to
   `PLURAL_CATEGORIES`.

Then add the tag to Table 1 and Table 5 of this guide, and add
`common/locales/<tag>/` to the `[files] exclude` list in `typos.local.toml`.
The spelling gate checks tracked text against an English dictionary, so it
would otherwise report the translation's words as misspellings. Leave English
variants checked.

### Step 5: Validate

Run the parity tests on their own while iterating:

```sh
cargo test -p whitaker-common --test i18n_catalogue_parity
```

A failure lists every finding with its locale and message, for example
`` is: `module_max_lines.help` drops `$module` ``. Before committing, run the
repository gates in order: `make fmt`, `make check-fmt`, `make lint`,
`make test`, and `make markdownlint`.

### Step 6: Read the rendered output

Technical gates do not show whether a sentence reads well. Render the locale
against real code by setting the variable for one run in a workspace that uses
Whitaker:

```sh
DYLINT_LOCALE=is cargo dylint --all
```

Check that each diagnostic reads naturally, that interpolated names and numbers
sit correctly in the sentence, and that singular and plural counts agree.

### Step 7: Compiler output

Five lint crates render one fixture in Welsh, and three of them also in Gaelic,
from directories named `ui-cy` and `ui-gd` beside the crate's `ui` directory.
Each holds a Rust file and the exact `.stderr` the compiler should print.
`no_std_fs_operations` also renders its fixture in Arabic, Hindi, Japanese, and
Russian (`ui-ar`, `ui-hi`, `ui-ja`, `ui-ru`), one locale per script family the
later catalogues introduced.

A new locale is covered without a fixture by
`every_locale_renders_the_lint_messages` in
`crates/no_std_fs_operations/src/diagnostics.rs`. That test resolves the lint's
messages in every shipped locale through `resolve_messages`, the function the
lint calls when it emits a diagnostic, and fails when a locale falls back to
the built-in English messages, loses a frozen token, or copies the English
text. A locale with an active reviewer may also add a `ui-<tag>` directory and
a test function that mirrors the existing ones in
`crates/no_std_fs_operations/src/tests/ui.rs`. To create its `.stderr`, run the
new test once: the failure prints the normalized compiler output to copy into
the file.

### Updating an existing locale

When a source message changes, change the same message in every locale, or
record the lag as a known gap in `KNOWN_MISSING_KEYS`. When only a translation
changes, update the glossary table in the same pull request if a listed term is
affected, and re-bless any `ui-<tag>` fixture that renders the message.

## 8. Right-to-left locales

Arabic, Persian, and Hebrew ship right-to-left catalogues. Whitaker's policy
for them differs from the usual Fluent advice, for two reasons that come from
where the text is displayed.

First, Fluent normally wraps each interpolated value in the isolation controls
U+2068 FIRST STRONG ISOLATE and U+2069 POP DIRECTIONAL ISOLATE. Whitaker
removes both from every rendered string: `normalize_fluent_output` in
`common/src/i18n/diagnostics.rs` strips them so that diagnostics and their
golden test files contain no invisible characters. Interpolated values are
therefore not isolated.

Second, a diagnostic is never a paragraph of its own. The compiler prints each
string after left-to-right text of its own, such as `warning:` or `= help:`,
and beside source code. The first strong character of the line is always Latin,
so the terminal lays the line out left to right whatever the message begins
with.

The policy that follows is:

- Do not put any bidirectional control character in a catalogue. That covers
  U+200E LEFT-TO-RIGHT MARK, U+200F RIGHT-TO-LEFT MARK, U+061C ARABIC LETTER
  MARK, the embeddings and overrides U+202A to U+202E, and the isolates U+2066
  to U+2069. A leading U+200F, which other projects use to set the paragraph
  direction, has no effect here because the message does not start the line.
- Write plain text in the natural order of the language. Keep code spans,
  identifiers, and placeables as they are.
- Where the language's own orthography offers a visible device that keeps a
  particle attached to a Latin token, such as the Hebrew hyphen after a prefix,
  use it. The glossary records such conventions per locale.

The structural parity test rejects every control listed above in every
catalogue, not only the right-to-left ones.

This policy has a known cost. In a terminal that applies the bidirectional
algorithm, a line that mixes right-to-left prose with several Latin identifiers
can display those identifiers in an unexpected visual order, although the
logical text is correct. No test covers visual rendering, and the catalogues
have not been checked in a bidirectional terminal. Restoring isolation for
right-to-left locales would need a change to `normalize_fluent_output` and to
the golden files, and is proposed follow-up work.

## 9. Quality checklist

Technical checks, which the parity tests verify:

- [ ] The locale directory holds the same 13 files as `en-GB`.
- [ ] Every message, attribute, and term in `en-GB` is present, and none is
      added.
- [ ] Every file parses without a syntax error.
- [ ] Each message uses only the variables it receives, and drops none.
- [ ] Each plural select lists exactly the locale's categories.
- [ ] `$handling` keeps its three variants.
- [ ] Frozen tokens are unchanged.
- [ ] No bidirectional control characters are present.
- [ ] No translated string is a copy of the source.
- [ ] Every string renders for a range of counts.

Linguistic checks, which only a fluent reader can verify:

- [ ] Each message means what the source means.
- [ ] Terms match the locale's glossary section.
- [ ] One address form and one instruction form are used throughout.
- [ ] Each plural variant agrees with its number.
- [ ] English phrases supplied by lints sit in a slot that needs no inflection.
- [ ] Rendered diagnostics read naturally beside real code.

Neither list substitutes for the other.

## 10. Validation during build and CI

No localization check runs at compile time. The build embeds whatever is in
`common/locales/`, including a file with a syntax error or a missing message.
All validation is in tests, which `make test` runs locally and in continuous
integration (CI):

- **Missing or extra file**: reported as `file set differs from en-GB`.
- **Syntax error**: reported with the parser's error list for the file.
- **Missing or unknown key**: reported as `is missing` or
  `does not exist in the source catalogue`.
- **Variable mismatch**: reported as `uses $x, which no call site supplies`
  or `drops $x`.
- **Plural mismatch**: reported with the variants found and the categories
  expected.
- **Altered literal**: reported as `lost the literal`.
- **Registry drift**: the loader or the directory tree disagrees with
  `EXPECTED_TAGS`.

Known gaps in validation:

- Nothing checks that a translation is correct or natural language.
- Nothing checks the wording of `.ftl` comments.
- Compiler output is checked byte for byte only for `cy`, `gd`, `ar`, `hi`,
  `ja`, and `ru`, and only for the lints that carry those fixtures. Every other
  locale is resolved through one lint's emission path, not rendered by the
  compiler, and the other lints' messages are checked only by the parity tests.
  A fixture for every locale and lint would add one serial compiler run each;
  that cost is why the coverage stops at one locale per script family.
- The directory `common/tests/i18n_quality/` holds an older suite that is not
  a Cargo test target and does not run. The parity tests supersede the checks
  it describes.

## 11. Testing translations

| Test                                                          | What it covers                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `common/tests/i18n_catalogue_parity/`                         | Registry oracle, file and key parity, variables, plural categories, literals, bidirectional controls, rendering of every string, selection of whole tags, per-message fallback; `rule_tests.rs` feeds each rule a defective fixture to prove it still fires; `properties.rs` checks the plural-category table and Fluent's selection against the plural engine for boundary and generated counts in every locale, and that locale matching ignores case and rejects unshipped tags |
| `common/src/i18n/tests.rs`                                    | Locale resolution, argument interpolation, removal of isolation marks                                                                                                                                                                                                                                                                                                                                                                                                              |
| `common/src/i18n/selection.rs` (unit tests)                   | Candidate order across environment, configuration, and fallback                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `common/tests/i18n_behaviour.rs`                              | Behaviour scenarios for fallback, Welsh and Gaelic lookups, and missing messages                                                                                                                                                                                                                                                                                                                                                                                                   |
| `common/tests/localizer_helpers.rs`                           | Locale choice from `DYLINT_LOCALE` and configuration; fallback reporting                                                                                                                                                                                                                                                                                                                                                                                                           |
| `common/tests/i18n_packaging.rs`                              | The fallback catalogue is present in the packaged crate                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `ui-cy`, `ui-gd`, and `ui-fallback` directories               | Compiler output in Welsh, in Gaelic, and for an unsupported tag, for the lint crates that carry them                                                                                                                                                                                                                                                                                                                                                                               |
| `ui-ar`, `ui-hi`, `ui-ja`, and `ui-ru` directories            | Compiler output in Arabic, Hindi, Japanese, and Russian for `no_std_fs_operations`                                                                                                                                                                                                                                                                                                                                                                                                 |
| `crates/no_std_fs_operations/src/diagnostics.rs` (unit tests) | Every shipped locale resolves that lint's messages through its emission path without fallback, keeps its frozen tokens, and differs from English                                                                                                                                                                                                                                                                                                                                   |

_Table 6: Tests that cover localization._

Run them all with:

```sh
make test
```

`make test` does not run the installer behaviour tests or the doctests; neither
touches localization.

## 12. Resources

- [Localization style guide](localization-styleguide.md): voice, tone, and
  writing mechanics.
- [Localization glossary](localization-glossary.md): terminology and
  per-locale register.
- [Developer's guide](developers-guide.md#localized-diagnostics): the
  `Localizer` API for lint authors.
- [Project Fluent](https://projectfluent.org/) and the
  [Fluent syntax guide](https://projectfluent.org/fluent/guide/).
- [Fluent selectors](https://projectfluent.org/fluent/guide/selectors.html),
  for select expressions and plural categories.
- [CLDR plural rules](https://cldr.unicode.org/index/cldr-spec/plural-rules),
  noting that the installed engine implements release 37.
- [Unicode isolation in Fluent](https://github.com/projectfluent/fluent.js/wiki/Unicode-Isolation),
  for background to section 8.
