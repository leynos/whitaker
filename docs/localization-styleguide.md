# Whitaker localization style guide

This document defines Whitaker's house style for localized diagnostic text and
explains how that style carries into every translation. It is written for
translators, reviewers, and lint authors who write the `en-GB` source messages.
The base guidance applies to all languages. The per-locale terminology and
register notes live in the [localization glossary](localization-glossary.md),
and the Fluent mechanics, the list of shipped locales, the fallback policy, and
the validation gates live in the [translator's guide](translators-guide.md).

The guide covers one product surface: the diagnostics Whitaker's lints emit
through the Rust compiler. Each diagnostic is a Fluent message with a primary
value and `.note`, `.help`, and, for some lints, `.label` and `.suggestion`
attributes. The `whitaker` command-line interface, the installer, and the log
output are not localized, so this guide sets no rules for them.

The distinction that organizes this guide is the standard one between voice and
tone: voice is the product's constant personality, while tone varies with
context.[^1] Translators keep the voice fixed and adapt the tone to the
expectations of the target language.

## Voice

Whitaker's voice is clear, calm, capable, and respectful. A diagnostic
interrupts someone who is trying to build their code, so it has to earn the
interruption by being exact and useful.

In practice this means:

- Messages state facts about the code. They do not blame the author, dramatize
  the finding, pad with filler, or claim more than the lint established.
- Diagnostics are concise and actionable. They name the function, module,
  attribute, type, or value concerned, and they give a correction only where
  the source message gives one.
- Diagnostics never joke, and they never adopt promotional language.

This voice is invariant. A translation that reads as bossy, jokey, apologetic,
or ceremonial has changed the product, not just the language.

## Tone by content type

Tone shifts with the role a string plays in the rendered diagnostic. The
compiler frames each part, as in this abridged `en-GB` rendering of
`no_unwrap_or_else_panic`, which omits the source excerpts:

```plaintext
error: Replace unwrap_or_else on `std::result::Result<i32, &str>` with a non-panicking fallback.
note: The closure supplied to unwrap_or_else triggers a panic.
   = help: Propagate the error or use expect with a descriptive message instead of panicking.
```

The source catalogue contains these content types:

- **Primary message** (the message value, for example `module_max_lines`):
  one complete sentence that states the finding or the change required. The
  source uses a direct instruction for some lints ("Avoid calling expect…") and
  a statement for others ("Module … spans … lines"). Keep whichever shape the
  source uses for that message.
- **Note** (`.note`): a factual explanation of why the lint fired or why the
  pattern matters. Declarative, never an instruction.
- **Help** (`.help`): the remediation. Imperative in English; use the target
  locale's natural instruction form, as recorded in its glossary section.
- **Span label** (`.label`): a short description attached to a highlighted
  region of code. `bumpy_road_function.label` is a full sentence;
  `string_continuation_style.label` is a lower-case noun phrase without a full
  stop. Match the source's shape for each.
- **Suggestion label** (`.suggestion`): a lower-case imperative fragment
  without terminal punctuation that introduces a suggested edit, such as
  "replace with a `concat!()` expression". Keep it a fragment.
- **Shared helpers** (`common-*` messages and `-term-*` terms in
  `common.ftl`): short, reusable wording. Translate each so that it reads
  correctly in isolation.

When translating, identify the content type first and reuse the pattern the
locale has already established for that type, instead of translating each
message in isolation.

Whitaker also carries built-in English wording in Rust for the case where a
lookup fails. That wording is a safety net, not a catalogue entry, and it is
not translated.

## Formality and register

The base register is professional plain language with moderate formality.
Prefer short sentences and direct instructions. Avoid slang, jokes, idioms,
culturally specific metaphors, and needlessly ceremonial phrasing.

Register does not translate literally; each locale renders the same
professional distance using its own conventions.[^2] The rules are:

- Use the natural professional register of the target locale for developer
  tools, not a literal translation of English formality markers.
- Where the language distinguishes formal and informal address, use the form
  that established developer tooling uses in that locale. The per-locale
  sections of the [glossary](localization-glossary.md) record the choice.
- Never mix address forms within a locale. One locale, one form.
- Prefer constructions that avoid gendering the reader or third parties. Where
  the language forces grammatical gender, prefer the locale's accepted neutral
  or generic convention over invented forms, and never assume the gender of the
  author of the code.
- Honorific systems follow the same rule: use the standard polite register of
  technical documentation, without humble or exalted forms.
- Record a choice that is genuinely unsettled in the locale's glossary section
  for review, instead of settling it silently in the catalogue.

## Usage rules

Do:

- Keep messages concise, concrete, and useful at the point where the code is
  flagged.
- Translate explanatory prose naturally while preserving its meaning and the
  order of its diagnostic content: the finding, then the value concerned, then
  any remediation.
- Preserve frozen tokens exactly. These are Rust paths and items (`std::fs`,
  `cap_std::fs::Dir`, `camino::Utf8Path`, `unwrap_or_else`, `expect`, `None`,
  `Err`), syntax (`concat!()`, `//!`), lint names, and Fluent placeables such as
  `{ $module }`. The glossary lists them.
- Keep the distinctions Whitaker's lints depend on: a doc comment is not an
  attribute, an inner doc comment is not an outer one, a module is not a crate,
  and a limit is not a threshold.
- Follow the [glossary](localization-glossary.md) for product names, technical
  terminology, and the per-locale term mappings.
- Keep repeated patterns parallel: translate a recurring source pattern, such
  as "… or move the code into a test", the same way each time it occurs.

Do not:

- Translate, transliterate, or inflect the written form of a product name, a
  Rust identifier, a lint name, or a message key.
- Add, rename, or invent a Fluent placeable. The
  [translator's guide](translators-guide.md) states which variables each
  message receives and the narrow cases in which one may be left out.
- Turn an actionable diagnostic into a vague statement, or invent a warning,
  remediation, or guarantee that is absent from the source.
- Replace a precise technical term with a near-synonym when Whitaker
  distinguishes the concepts.
- Add humour, idioms, gender assumptions, or culturally specific metaphors.
- Expand or contract meaning to suit the rhythm of the target language.
  Fidelity to the diagnostic content outranks elegance.

Some values arrive from the lint as fixed English phrases, for example the
`$name` of `conditional_max_n_branches` ("if condition"). A catalogue cannot
translate them. Build the sentence so that the phrase needs no inflection or
agreement, and set it off with the locale's quotation marks. The translator's
guide lists every such variable.

## Grammar and mechanics across locales

- **Word order and agreement.** Placeables move with the syntax of the target
  language. A placeable is never locked to its English position, but its name
  and braces are literal.
- **Sentence shape.** The source favours one statement per sentence. Keep that
  structure; do not merge or split sentences, even where the target language
  would tolerate it.
- **Text expansion.** Translations may legitimately run longer than the
  source.[^3] Span labels sit beside source code in a terminal, so keep
  `.label` and `.suggestion` text as compact as the language allows.
- **Punctuation.** Apply the target locale's rules for quotation marks,
  spacing, and terminal punctuation, such as French spacing before two-part
  punctuation or full-width punctuation in Chinese and Japanese. Punctuation
  inside code, paths, identifiers, and placeables is frozen, and code spans
  keep their ASCII backticks in every locale.
- **Numbers and units.** Numbers reach the catalogue through placeables and
  are formatted by the program, not by the translator. Do not re-spell or
  regroup them. A translation chooses only the wording around the number.
- **Capitalization.** Follow the rules of the target language instead of
  mirroring English casing. Identifiers keep their exact case in every script,
  including in locales such as Turkish whose casing rules would otherwise alter
  `i` and `I`.
- **Plurals.** Use the plural categories the target language requires, adding
  or removing variants as the language demands. The translator's guide owns the
  category table and the select-expression syntax.
- **Scripts and transliteration.** Keep loan words in the script that the
  locale's technical writing conventionally uses. Never transliterate
  identifiers, keys, or code.

## Locale integrity

- Treat `common/locales/en-GB/` as the source catalogue. Preserve every
  message identifier, attribute, and term.
- Preserve valid Fluent syntax, attributes, terms, select expressions, and
  plural variants.
- Preserve meaningful locale variants. Do not collapse `en-US` into `en-GB`,
  `es-419` into `es-ES`, `pt-BR` into `pt-PT`, or `zh-Hans` into `zh-Hant`.
  Regional and script variants exist because their vocabulary, orthography, or
  register differ. The [translator's guide](translators-guide.md) owns the list
  of shipped tags.
- Do not add direction marks or other bidirectional control characters to a
  right-to-left catalogue. The translator's guide explains the directionality
  policy and why it differs from other Fluent projects.
- Do not translate locale tags, message identifiers, lint names, or the
  configuration keys and environment variables a user types, even when they
  appear inside otherwise translatable prose.

## Machine-readable output

Tools consume Whitaker's diagnostics as well as people: the compiler can emit
them as JSON, editors attach them to source lines, and `#[expect(...)]`
attributes name lints. The boundary between translated prose and stable
interface is:

- The text of the primary message, notes, help, labels, and suggestion labels
  translates, including when the compiler wraps that text in a JSON `message` or
  `rendered` field.
- The lint name that appears in the diagnostic code, in `#[allow(...)]` and
  `#[expect(...)]` attributes, and in configuration never translates. Nor do
  severity levels, spans, file paths, or the replacement code a suggestion
  applies.
- The words the compiler itself supplies, such as `warning`, `note`, and
  `help`, are outside Whitaker's catalogues and stay as the compiler prints
  them.

## Quality checklist

Before submitting a locale, confirm:

1. Every message conveys the same finding, value, and remediation as the
   source, with nothing added and nothing lost.
2. Terminology matches the glossary, including the locale's own section.
3. One address form and one instruction form are used throughout.
4. Repeated source patterns are translated identically each time.
5. Every frozen token and placeable survives exactly, and no English phrase
   supplied by a lint has been inflected.
6. Punctuation, quotation marks, and spacing follow the locale, except inside
   frozen tokens.
7. Span labels and suggestion labels are compact and keep the source's shape.
8. No humour, idiom, metaphor, or invented content has crept in.
9. The technical gates in the [translator's guide](translators-guide.md) pass.
   They check structure, not language; passing them does not replace review by
   a fluent reader.

When a source message is ambiguous, inspect the lint that emits it and record
the question for review, instead of guessing or silently broadening its meaning.

[^1]: The voice and tone split is standard localization practice; see, for
    example,
    [Mailchimp's content style guide](https://styleguide.mailchimp.com/voice-and-tone/).

[^2]: Per-language register conventions for software are documented in the
    [Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides),
    which this project uses as a reference corpus for locale register.

[^3]: On expansion and its effect on layout, see
    [W3C's guidance on text size in translation](https://www.w3.org/International/articles/article-text-size).
