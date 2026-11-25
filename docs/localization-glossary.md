# Whitaker localization glossary

This document is the source of truth for Whitaker terminology across every
locale. It records the preferred, allowed, and forbidden forms of each
source-language term, explains which terms resist translation and why, and sets
out per-locale terminology and register so that translators and reviewers apply
the same vocabulary consistently. It complements the
[translator's guide](translators-guide.md), which owns the list of shipped
locales, the Fluent mechanics, and the fallback policy, and the
[localization style guide](localization-styleguide.md), which owns voice, tone,
and writing mechanics.

## Schema

Each row in a terminology table represents one term record. The columns are:

- `title`: the canonical term as it appears in the source catalogue. Locale
  tables reuse the same `title`, so each mapping can be traced to its source
  record.
- `preferred`: the form translators must use in the stated context.
- `allowed`: acceptable alternatives, separated by semicolons. A qualifier in
  parentheses limits an alternative to one context.
- `forbidden`: forms that must not appear, separated by semicolons.

An em dash (—) means the set is empty. It never means that a decision is
pending; open questions are recorded in the prose around a table.

Keep forbidden values conservative. Add a value only for an observed
misspelling, an identifier variant that would break a literal interface, or a
collision between distinct Whitaker concepts.

## Source terminology (en-GB)

Whitaker's source catalogue is the set of Fluent files under
`common/locales/en-GB/`. It is written in British English with Oxford spelling
("-ize", "-our"), and the `en-GB` terms below are authoritative for message
content. Per-locale sections map the difficult subset of these terms into each
shipped locale.

### Product and ecosystem names

These names are invariant in every locale. They are never translated,
transliterated, or respelt, and no affix may alter their written form, although
the surrounding text follows the grammar of the target language.

| title      | preferred    | allowed                                            | forbidden                 |
| ---------- | ------------ | -------------------------------------------------- | ------------------------- |
| Whitaker   | `Whitaker`   | `whitaker` (executable, package, and code path)    | —                         |
| Dylint     | `Dylint`     | `dylint` (package name); `cargo dylint` (command)  | —                         |
| Rust       | `Rust`       | —                                                  | —                         |
| Cargo      | `Cargo`      | `cargo` (executable or literal command)            | —                         |
| Clippy     | `Clippy`     | —                                                  | —                         |
| Fluent     | `Fluent`     | `Project Fluent`; `Fluent Translation List`; `FTL` | —                         |
| cap-std    | `cap_std`    | `cap-std` (package name)                           | `capstd`                  |
| camino     | `camino`     | —                                                  | `Camino`                  |
| Bumpy Road | `Bumpy Road` | `bumpy_road_function` (lint name)                  | `bumpy road` (lower case) |
| CLDR       | `CLDR`       | `Unicode CLDR`                                     | —                         |

_Table 1: Product and ecosystem names._

### Rust language concepts

These terms name Rust constructs that Whitaker's lints inspect. Each has a
precise meaning in the Rust reference, and several lints exist only to enforce
a distinction between two of them.

| title             | preferred               | allowed                      | forbidden                      |
| ----------------- | ----------------------- | ---------------------------- | ------------------------------ |
| lint              | `lint`                  | `Whitaker lint`              | `linter` (for one lint)        |
| crate             | `crate`                 | `downstream crate`           | `package` (for a crate)        |
| module            | `module`                | —                            | `file` (for a module)          |
| function          | `function`              | `public function`            | `method` (for a free function) |
| helper function   | `helper function`       | `helper`                     | —                              |
| closure           | `closure`               | —                            | `lambda`; `callback`           |
| macro expansion   | `macro expansion`       | `expansion`                  | —                              |
| attribute         | `attribute`             | `outer attribute`            | `annotation`; `decorator`      |
| outer attribute   | `outer attribute`       | —                            | `inner attribute`              |
| doc comment       | `doc comment`           | `documentation comment`      | `docstring`                    |
| inner doc comment | `inner doc comment`     | —                            | `outer doc comment`            |
| test              | `test`                  | `test-only code`             | —                              |
| panic             | `panic` (noun and verb) | `panicking`; `non-panicking` | `crash`; `exception`           |
| variant           | `variant`               | —                            | —                              |
| fallback          | `fallback`              | `non-panicking fallback`     | —                              |
| literal           | `literal`               | `string literal`             | —                              |
| cooked string     | `cooked string`         | —                            | `raw string`                   |
| escaped newline   | `escaped newline`       | —                            | —                              |
| physical line     | `physical line`         | `source line`                | —                              |

_Table 2: Rust language terms._

### Analysis and policy vocabulary

These terms describe what a lint measures or enforces.

| title                | preferred              | allowed                              | forbidden                               |
| -------------------- | ---------------------- | ------------------------------------ | --------------------------------------- |
| cognitive complexity | `cognitive complexity` | —                                    | `cyclomatic complexity`                 |
| complexity bump      | `complexity bump`      | `bump`                               | —                                       |
| threshold            | `threshold`            | —                                    | `limit` (for the bump threshold)        |
| limit                | `limit`                | `configured limit`                   | `threshold` (for a line or score limit) |
| branch               | `branch`               | `branching`                          | —                                       |
| conditional          | `conditional`          | `nested conditional logic`           | —                                       |
| cluster              | `cluster`              | —                                    | —                                       |
| test coverage        | `test coverage`        | —                                    | —                                       |
| example section      | `example section`      | `example`                            | —                                       |
| source-line join     | `source-line join`     | —                                    | —                                       |
| fragment boundary    | `fragment boundary`    | —                                    | —                                       |
| capability           | `capability`           | `capability-based filesystem policy` | `permission`; `capacity`                |
| working directory    | `working directory`    | `ambient working directory`          | `folder`                                |
| error path           | `error path`           | —                                    | —                                       |
| diagnostic           | `diagnostic`           | `diagnostics`                        | `error message` (for any diagnostic)    |

_Table 3: Analysis and policy terms._

### Localization concepts

These terms appear in the documentation set and in catalogue comments, not in
rendered diagnostics.

| title             | preferred           | allowed                                          | forbidden                |
| ----------------- | ------------------- | ------------------------------------------------ | ------------------------ |
| locale            | `locale`            | `locale tag`                                     | `language` (for the tag) |
| source catalogue  | `source catalogue`  | `en-GB catalogue`                                | —                        |
| message           | `message`           | `message identifier`; `message key`              | —                        |
| Fluent attribute  | `Fluent attribute`  | `attribute` (where no Rust attribute is in view) | —                        |
| placeable         | `placeable`         | `variable` (for a `{ $name }` reference)         | `placeholder text`       |
| plural category   | `plural category`   | `CLDR plural category`                           | —                        |
| select expression | `select expression` | —                                                | —                        |
| fallback locale   | `fallback locale`   | —                                                | —                        |

_Table 4: Localization terms._

### Frozen tokens

These tokens are code. They keep their exact spelling, case, and punctuation in
every locale and every script, whether or not the source wraps them in
backticks.

| title              | preferred                                  | allowed                        | forbidden                          |
| ------------------ | ------------------------------------------ | ------------------------------ | ---------------------------------- |
| `expect`           | `expect`                                   | —                              | `Expect`; `expect()`               |
| `unwrap_or_else`   | `unwrap_or_else`                           | —                              | `unwrap or else`; `unwrap-or-else` |
| `std::fs`          | `std::fs`                                  | —                              | `std fs`; `std.fs`                 |
| `cap_std::fs::Dir` | `cap_std::fs::Dir`                         | —                              | —                                  |
| `camino::Utf8Path` | `camino::Utf8Path`                         | `Utf8PathBuf` (the owned type) | `UTF8Path`                         |
| `concat!()`        | `concat!()`                                | —                              | `concat()`; `concat!`              |
| `None`             | `None`                                     | —                              | `none`; `null`                     |
| `Err`              | `Err`                                      | —                              | `Error`; `err`                     |
| `//!`              | `//!`                                      | —                              | `///`                              |
| lint names         | as written, for example `module_max_lines` | —                              | any translated or respaced form    |
| `DYLINT_LOCALE`    | `DYLINT_LOCALE`                            | —                              | —                                  |
| `dylint.toml`      | `dylint.toml`                              | —                              | —                                  |

_Table 5: Frozen tokens._

### Spelling-sensitive source words

The source catalogue follows Oxford spelling. These are the words in rendered
messages whose spelling differs between English variants.

| title      | preferred    | allowed | forbidden    |
| ---------- | ------------ | ------- | ------------ |
| behaviour  | `behaviour`  | —       | `behavior`   |
| recognized | `recognized` | —       | `recognised` |

_Table 6: Spelling-sensitive words in the source catalogue._

## Localizability notes

The terms below are the ones most likely to go wrong in translation. Each note
explains the hazard; the per-locale sections apply these notes to each shipped
locale.

- **`lint`.** A lint is one rule that inspects code, and a lint's name is an
  identifier users type in `#[expect(...)]`. Most languages keep the English
  loan. A word meaning "linter" names the tool, not the rule, and a word
  meaning "warning" names one finding, not the rule.
- **`crate` versus package versus module.** A crate is Rust's compilation
  unit; a Cargo package may hold several crates; a module is a namespace inside
  a crate. Everyday words for "box" or "library" lose the distinction. Most
  locales keep `crate` as a loan.
- **Doc comment, inner doc comment, and attribute.** Three lints turn on these
  distinctions. A doc comment (`///`) documents the item that follows it; an
  inner doc comment (`//!`) documents the item that encloses it; an outer
  attribute (`#[...]`) is not documentation at all. "Inner" and "outer"
  describe position relative to the item, not visibility, so words meaning
  "internal" and "external" in the sense of private and public mislead.
- **`attribute` in two senses.** Rendered messages mean a Rust attribute.
  Catalogue comments and this documentation also mention Fluent attributes
  (`.note`, `.help`). Qualify the word wherever both could be in view.
- **`panic`.** A Rust panic is a specific, recoverable unwinding mechanism. It
  is not a crash, an exception, or an emotional state, and English uses the one
  word as noun and verb. Many languages keep the loan for the noun and need a
  separate construction for the verb; choose one of each per locale.
- **`fallback`.** In `no_unwrap_or_else_panic` the fallback is the value or
  code path used when a `Result` or `Option` is empty. The fallback locale is a
  different idea, and a locale may need two different words.
- **`closure`.** The Rust term for an anonymous function that captures its
  environment. Words for "lambda" or "callback" are near-synonyms that Rust
  documentation does not use.
- **`cooked string` and `escaped newline`.** A cooked string is the compiler's
  name for an ordinary, non-raw string literal, in which escapes are processed.
  It has no culinary meaning, and few languages have an established rendering.
  An escaped newline is a backslash at the end of a line, which removes the
  line break and the indentation that follows. A locale may prefer a
  descriptive phrase to a literal calque of either.
- **`complexity bump`.** The term comes from the "Bumpy Road" code smell: a
  bump is one cluster of nested conditional logic inside a function. It is a
  metaphor, so a literal translation may read as a physical bump or an
  increase. The shipped Welsh and Gaelic catalogues keep the English word in
  quotation marks.
- **`threshold` versus `limit`.** The bump threshold is a tuning value that
  smoothed complexity must exceed; a limit is a configured maximum for lines,
  branches, or a score. Keep two words where the language has them.
- **`branch`.** In `conditional_max_n_branches` a branch is one operand of a
  compound condition joined by `&&` or `||`. It is not a version-control
  branch, and it is not an arm of a `match`.
- **`capability`.** The word is the security term of art: an unforgeable
  handle that grants access to one resource. Words meaning "capacity" (volume)
  or "ability" (skill) are false friends in several languages.
- **`working directory`.** "Ambient" describes authority that a program holds
  implicitly. Translate the idea that the directory is implicit, not a word for
  "surrounding atmosphere".
- **English phrases supplied by lints.** Some placeables carry fixed English
  phrases, such as "if condition" or "a fenced code block". They are not
  glossary terms, and a catalogue cannot translate them; the
  [translator's guide](translators-guide.md) lists them and the
  [style guide](localization-styleguide.md) says how to write around them.

## Locale terminology

Each section below records how the difficult subset of the source terminology
is rendered in one shipped locale, together with register notes and the loan
words that locale conventionally keeps. Terms listed as invariant in Table 1
and the frozen tokens in Table 5 stay invariant in every locale and are not
repeated.

The sections differ in how far their content has been reviewed, and readers
should weigh them accordingly:

- The Welsh (`cy`) and Scottish Gaelic (`gd`) sections describe catalogues
  that shipped before this glossary existed. Their tables record the vocabulary
  those catalogues use.
- Every other section, and the catalogue it describes, was drafted by an AI
  model in November 2026 and has not been reviewed by a fluent reader. Each
  table records the vocabulary that locale's catalogue uses, so the two stay
  consistent, but every row is provisional until a reviewer who reads the
  language confirms it. The [maintenance](#maintenance) section explains how to
  record that review.

Register conventions follow the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
where one exists for the language, and Rust vocabulary follows the community
translations of _The Rust Programming Language_ listed in
[its translations appendix](https://doc.rust-lang.org/book/appendix-06-translation.html)
where one exists. Neither source was consulted line by line, so a section that
names a convention is recording the drafter's understanding of it, not a
verified citation.

The Arabic, Hindi, and Thai sections suspend two Markdown lint rules, line
length (MD013) and table column alignment (MD060). Those scripts use combining
marks, which the repository formatter measures by display width and the linter
counts as characters, so the two tools cannot both be satisfied there. The
formatter's layout is kept, and the rules stay enabled for every other section.

### Arabic (`ar`)

<!-- markdownlint-disable MD013 MD060 -->

Arabic diagnostics are written in Modern Standard Arabic, never a regional
dialect, in line with the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides).
Arabic has no T–V distinction; the standard register itself carries the
professional distance. `.help` text uses the direct imperative (`استخرج`, `قسّم`,
`انقل`) in the conventional generic masculine singular of software
instructions, while primary messages and notes state the condition with a
verbal or nominal sentence (`توجد`, `يجب أن تبدأ`). A verbal-noun rewrite that
avoids the gendered imperative is an open choice for a reviewer. Diacritics
appear only where they prevent a misreading (`يُحدث`, `مُصدَّرة`), and the
catalogue uses Arabic punctuation (`،` and `؛`) with guillemets (`«»`) as
quotation marks. Every plural select lists all six categories, because the
counted noun changes form across them: no numeral for zero, one, and two
(`سطر واحد`, `سطرين`), a genitive plural for three to ten (`أسطر`), an
accusative singular for eleven to ninety-nine (`سطرًا`), and a genitive singular
otherwise (`سطر`). Directionality mechanics are owned by the translator's guide
(`translators-guide.md`, section 8).

Native terms are established for `دالة`, `وحدة`, `فرع`, `عتبة`, `اختبار`,
`سمة`, and `دليل العمل`. `lint` stays in Latin script, glossed as `قاعدة lint`
where a countable noun is needed, and `ماكرو` is a adapted loan. Identifiers
and code stay in Latin script.

`سمة` renders a Rust attribute, but Arabic Rust writing also uses it for a
trait; the catalogue never translates the word trait, so the collision cannot
arise inside a message. `متغاير` renders an enum `variant` and must not be
shortened to `متغير`, which means a variable. `وحدة` also means a unit of
measure, so it always appears with the module name or in a module-specific
sentence. `هلع` for `panic`, `مغلق` for `closure`, and `صندوق` for `crate`
follow usage seen in Arabic Rust material but are not settled, and the English
loan remains an allowed alternative for each. `سلسلة نصية معالَجة` for
`cooked string`, `سطر جديد مهرّب` for `escaped newline`, `نتوء تعقيد` for
`complexity bump`, `تعليق توثيقي داخلي` for `inner doc comment`, and `قدرة` for
`capability` are coinages with no established Arabic rendering.

Worked example, from `common/locales/ar/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [zero] لا تمتد الوحدة { $module } على أي سطر، والحد المسموح به هو { $limit }.
        [one] تمتد الوحدة { $module } على سطر واحد، متجاوزةً الحد المسموح به { $limit }.
        [two] تمتد الوحدة { $module } على سطرين، متجاوزةً الحد المسموح به { $limit }.
        [few] تمتد الوحدة { $module } على { $lines } أسطر، متجاوزةً الحد المسموح به { $limit }.
        [many] تمتد الوحدة { $module } على { $lines } سطرًا، متجاوزةً الحد المسموح به { $limit }.
       *[other] تمتد الوحدة { $module } على { $lines } سطر، متجاوزةً الحد المسموح به { $limit }.
    }
```

| title                | preferred            | allowed      | forbidden        |
| -------------------- | -------------------- | ------------ | ---------------- |
| lint                 | `lint`               | `قاعدة lint` | —                |
| crate                | `صندوق`              | `crate`      | `حزمة`           |
| module               | `وحدة`               | `وحدة نمطية` | —                |
| function             | `دالة`               | —            | —                |
| helper function      | `دالة مساعدة`        | —            | —                |
| closure              | `مغلق`               | `closure`    | —                |
| macro expansion      | `توسيع الماكرو`      | —            | —                |
| attribute            | `سمة`                | —            | `خاصية`          |
| outer attribute      | `سمة خارجية`         | —            | —                |
| doc comment          | `تعليق توثيقي`       | —            | —                |
| inner doc comment    | `تعليق توثيقي داخلي` | —            | —                |
| test                 | `اختبار`             | —            | —                |
| panic                | `هلع`                | `panic`      | —                |
| variant              | `متغاير`             | —            | `متغير`          |
| fallback             | `بديل احتياطي`       | `بديل`       | `نسخة احتياطية`  |
| cooked string        | `سلسلة نصية معالَجة`  | —            | `سلسلة نصية خام` |
| escaped newline      | `سطر جديد مهرّب`      | —            | —                |
| cognitive complexity | `تعقيد إدراكي`       | —            | —                |
| complexity bump      | `نتوء تعقيد`         | —            | —                |
| threshold            | `عتبة`               | —            | `حد`             |
| branch               | `فرع`                | —            | —                |
| test coverage        | `تغطية الاختبارات`    | —            | —                |
| working directory    | `دليل العمل`         | —            | `مجلد العمل`     |
| capability           | `قدرة`               | —            | `صلاحية`          |
| diagnostic           | `رسالة تشخيصية`      | —            | —                |

_Table 7: Arabic terminology._

<!-- markdownlint-enable MD013 MD060 -->

### Czech (`cs`)

Czech diagnostics address the reader with the formal second person plural
(`vykání`), written in lower case, which is the address form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for Czech software. Instructions in `.help` use the plural imperative
(`Extrahujte`, `Přesuňte`, `Rozdělte`); conditions are stated impersonally or
with passive participles (`Zjištěno`, `je uveden`) so that no message assigns
blame or assumes the reader's gender. Quotation marks are `„…“`.

`lint` and `crate` stay as English loans: `lint` declines as a masculine
inanimate noun (`lintu`, `linty`), while `crate` is left undeclined, which is
also how Czech Rust writing commonly treats it (see the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html)).
The remaining vocabulary has native or long-established forms: `modul`,
`funkce`, `uzávěr`, `makro`, `atribut`, `dokumentační komentář`, `varianta`,
`větev`, and `pracovní adresář`.

Czech numerals govern case: 1 takes the nominative or accusative singular, 2–4
the plural, 5 and above the genitive plural, and fractions the genitive
singular, so every counted sentence is a four-way select and the past
participle agrees as well (`Zjištěn`, `Zjištěny`, `Zjištěno`). English
fragments such as the conditional kind are placed in quotation marks after a
declinable head noun (`výraz „{ $name }“`), which carries the case ending
instead of the fragment. `cooked string` (`zpracovaný řetězec`),
`complexity bump` (`hrbol složitosti`), and `capability` in the security sense
(`schopnost`) are coinages with no established Czech rendering; `oprávnění` is
avoided for `capability` because it means a permission. `test` is always the
noun, `panika` is always the noun used with `vyvolat`, and `fallback` is
rendered `záložní řešení` for a fallback value or path and by the adjective
`záložní` for fallback diagnostics. `atribut` always means a Rust attribute;
the catalogues never mention Fluent attributes.

Worked example, from `common/locales/cs/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Modul { $module } má { $lines } řádek, což překračuje povolený limit { $limit }.
        [few] Modul { $module } má { $lines } řádky, což překračuje povolený limit { $limit }.
        [many] Modul { $module } má { $lines } řádku, což překračuje povolený limit { $limit }.
       *[other] Modul { $module } má { $lines } řádků, což překračuje povolený limit { $limit }.
    }
```

| title                | preferred                       | allowed                                            | forbidden                                   |
| -------------------- | ------------------------------- | -------------------------------------------------- | ------------------------------------------- |
| lint                 | `lint`                          | —                                                  | —                                           |
| crate                | `crate`                         | `balíček` (for a Cargo package only)               | `bedna`                                     |
| module               | `modul`                         | —                                                  | —                                           |
| function             | `funkce`                        | —                                                  | —                                           |
| helper function      | `pomocná funkce`                | —                                                  | —                                           |
| closure              | `uzávěr`                        | `closure`                                          | —                                           |
| macro expansion      | `expanze makra`                 | `rozvinutí makra`                                  | —                                           |
| attribute            | `atribut`                       | —                                                  | `vlastnost`                                 |
| outer attribute      | `vnější atribut`                | —                                                  | —                                           |
| doc comment          | `dokumentační komentář`         | —                                                  | —                                           |
| inner doc comment    | `vnitřní dokumentační komentář` | —                                                  | —                                           |
| test                 | `test`                          | —                                                  | —                                           |
| panic                | `panika`                        | `panic` (in code references)                       | —                                           |
| variant              | `varianta`                      | —                                                  | —                                           |
| fallback             | `záložní řešení`                | `záložní` (adjective, as in `záložní diagnostika`) | —                                           |
| cooked string        | `zpracovaný řetězec`            | —                                                  | `raw řetězec`                               |
| escaped newline      | `escapovaný konec řádku`        | —                                                  | —                                           |
| cognitive complexity | `kognitivní složitost`          | —                                                  | —                                           |
| complexity bump      | `hrbol složitosti`              | —                                                  | —                                           |
| threshold            | `práh`                          | —                                                  | `limit` (reserved for the configured limit) |
| branch               | `větev`                         | —                                                  | —                                           |
| test coverage        | `pokrytí testy`                 | —                                                  | —                                           |
| working directory    | `pracovní adresář`              | —                                                  | `složka`                                    |
| capability           | `schopnost`                     | —                                                  | `oprávnění`                                 |
| diagnostic           | `diagnostika`                   | `diagnostická zpráva`                              | —                                           |

_Table 8: Czech terminology._

### Welsh (`cy`)

The Welsh catalogue shipped before this glossary existed, and this section
records the vocabulary it uses. It addresses the reader with the formal or
plural imperative in `-wch` (`Symudwch`, `Rhannwch`, `Peidiwch â`), which is
the instruction form used throughout `.help` text; one message
(`no_std_fs_operations.help`) uses the verb-noun `Pasio` instead. Notes are
declarative statements with `Mae` or `Nid yw`. Welsh marks no formal and
informal distinction beyond the choice of the `-wch` form, and the catalogue
never addresses the reader as `ti`.

Welsh mutates the first letter of a word according to what precedes it, so a
term appears in several written forms: `cangen` is also written `gangen` and
`changen`, and `modiwl` becomes `fodiwl`. The table gives the unmutated
citation form. Interpolated identifiers are never mutated. Counted nouns need
care because Welsh numerals select gender, mutation, and sometimes the
construction `o` with a plural noun; the catalogue therefore writes
whole-sentence variants for all six plural categories, and the `branch_phrase`
helper described in the [translator's guide](translators-guide.md) renders
branch counts for this locale.

The catalogue keeps `lint`, `macro`, `panic`, and `bump` as English loans.
`lint` takes the Welsh plural `lintiau`, and `panic` is verbed as `panicio`.
Native terms cover the rest of the vocabulary.

Several choices are open questions for a Welsh-speaking reviewer, and are
recorded here instead of being settled silently:

- `capasiti` renders "capability" in the primary message of
  `no_std_fs_operations`, but the word means capacity; the note in the same
  message uses `gallu`. The source glossary lists "capacity" as a form to avoid.
- `clôd` renders "fallback" in `no_unwrap_or_else_panic`, where the sense
  intended is a substitute value. `wrth gefn` renders the fallback locale in
  `common.ftl`.
- `llinyn wedi'i goginio` is a literal calque of "cooked string".
- `sylw doc` and `sylw dogfen` both render "doc comment", and `cronfa`
  renders "crate" in one message only.
- `string_continuation_style.ftl` uses the ASCII apostrophe where every other
  file uses the typographic one.

Worked example, from `common/locales/cy/module_max_lines.ftl`:

```ftl
module_max_lines = Mae modiwl { $module } yn ymestyn i { $lines } o linellau ac yn torri’r terfyn o { $limit }.
```

| title                | preferred                  | allowed                | forbidden |
| -------------------- | -------------------------- | ---------------------- | --------- |
| lint                 | `lint`                     | `lintiau` (plural)     | —         |
| crate                | `cronfa`                   | —                      | —         |
| module               | `modiwl`                   | —                      | —         |
| function             | `swyddogaeth`              | —                      | —         |
| helper function      | `swyddogaeth gynorthwyol`  | `cod cymorth`          | —         |
| closure              | `caead`                    | —                      | —         |
| macro expansion      | `ehangiad macro`           | —                      | —         |
| attribute            | `priodoledd`               | —                      | —         |
| outer attribute      | `priodoledd allanol`       | —                      | —         |
| doc comment          | `sylw doc`                 | `sylw dogfen`          | —         |
| inner doc comment    | `sylw mewnol`              | —                      | —         |
| test                 | `prawf`                    | `profion` (plural)     | —         |
| panic                | `panic`                    | `panicio` (verb)       | —         |
| variant              | `amrywiad`                 | `achos` (for `None`)   | —         |
| fallback             | `wrth gefn`                | `clôd` (see above)     | —         |
| cooked string        | `llinyn wedi'i goginio`    | —                      | —         |
| escaped newline      | `llinell newydd ddianc`    | —                      | —         |
| cognitive complexity | `cymhlethdod gwybyddol`    | —                      | —         |
| complexity bump      | `bump cymhlethdod`         | —                      | —         |
| threshold            | `trothwy`                  | —                      | —         |
| branch               | `cangen`                   | `canghennau` (plural)  | —         |
| test coverage        | `cwmpas profion`           | —                      | —         |
| working directory    | `cyfeiriadur amgylcheddol` | —                      | —         |
| capability           | `gallu`                    | `capasiti` (see above) | —         |
| diagnostic           | `diagnosteg`               | —                      | —         |

_Table 9: Welsh terminology._

### Danish (`da`)

Danish diagnostics address the reader informally with `du`, the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
use for Danish software; the formal `De` is obsolete in this register and
never appears. Messages that demand a change and every `.help` attribute use
the bare imperative (`Flyt …`, `Opdel …`), so the pronoun surfaces only in
possessives such as `dine API'er`. Condition reports are declarative. Danish
comma rules are followed, including the comma before a coordinated second
imperative (`…, eller flyt koden ind i en test`). Quotation marks around
English fragments are the double typographic pair.

The catalogue keeps `lint`, `crate`, `closure`, `panic`, `fallback`, `literal`,
and `handle` as English loans, as Danish developer writing commonly does.
`modul`, `funktion`, `hjælpefunktion`, `attribut`, `variant`, `tærskelværdi`,
`gren`, `testdækning`, and `arbejdsmappe` are established native terms, and
`kapabilitet` is used as the adapted form of capability so that it can form
compounds (`kapabilitetsbaseret`).

The main hazards are these. `gren` renders a counted branch of a predicate and
must not be replaced by `branch`, which reads as a version-control branch; the
activity is `forgrening`. `attribut` is a common-gender noun here
(`den ydre attribut`), and a Fluent attribute needs the qualifier
`Fluent-attribut` in translators' notes. `panic` stays an uninflected loan
(`udløse panic`) so that it is not confused with the everyday `panik`. `grænse`
renders the configured limit and `tærskelværdi` the bump threshold.
`cooked streng`, `kompleksitetstop`, `indre dokumentationskommentar`, and
`escapet linjeskift` are coinages or ad hoc borrowings with no established
Danish rendering.

Worked example, from `common/locales/da/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Modulet { $module } fylder { $lines } linje og overskrider den tilladte grænse på { $limit }.
       *[other] Modulet { $module } fylder { $lines } linjer og overskrider den tilladte grænse på { $limit }.
    }
```

| title                | preferred                       | allowed                      | forbidden                                                |
| -------------------- | ------------------------------- | ---------------------------- | -------------------------------------------------------- |
| lint                 | `lint`                          | `lint-regel`                 | —                                                        |
| crate                | `crate`                         | —                            | `kasse`; `pakke` (a Cargo package is a distinct concept) |
| module               | `modul`                         | —                            | —                                                        |
| function             | `funktion`                      | —                            | —                                                        |
| helper function      | `hjælpefunktion`                | —                            | —                                                        |
| closure              | `closure`                       | —                            | `lukning`                                                |
| macro expansion      | `makroudvidelse`                | `makroekspansion`            | —                                                        |
| attribute            | `attribut`                      | —                            | —                                                        |
| outer attribute      | `ydre attribut`                 | —                            | —                                                        |
| doc comment          | `dokumentationskommentar`       | `doc-kommentar`              | —                                                        |
| inner doc comment    | `indre dokumentationskommentar` | —                            | —                                                        |
| test                 | `test`                          | —                            | —                                                        |
| panic                | `panic`                         | `udløse panic` (verb)        | `panik`                                                  |
| variant              | `variant`                       | —                            | —                                                        |
| fallback             | `fallback`                      | —                            | —                                                        |
| cooked string        | `cooked streng`                 | —                            | `kogt streng`                                            |
| escaped newline      | `escapet linjeskift`            | —                            | —                                                        |
| cognitive complexity | `kognitiv kompleksitet`         | —                            | —                                                        |
| complexity bump      | `kompleksitetstop`              | —                            | —                                                        |
| threshold            | `tærskelværdi`                  | —                            | `grænse` (reserved for the limit)                        |
| branch               | `gren`                          | `forgrening` (the activity)  | `branch`                                                 |
| test coverage        | `testdækning`                   | —                            | —                                                        |
| working directory    | `arbejdsmappe`                  | `arbejdskatalog`             | —                                                        |
| capability           | `kapabilitet`                   | —                            | `evne`                                                   |
| diagnostic           | `diagnosticering`               | `diagnosticeringsmeddelelse` | —                                                        |

_Table 10: Danish terminology._

### German (`de`)

German diagnostics address the reader with the formal `Sie`, the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
establish for German developer tooling, and never mix it with `du`. Primary
messages that demand a change and every `.help` attribute use the `Sie`
imperative (`Verschieben Sie …`, `Teilen Sie … auf`); messages and notes that
report a condition are plain declarative sentences without an addressee. The
short `.suggestion` label uses the infinitive, which is the usual German form
for a menu-like action. No sentence refers to the reader's gender, so no
gender-inclusive notation is needed. Nouns are capitalized by German rule,
including loans (`Lint`, `Crate`, `Closure`), and compounds that attach to a
frozen identifier are hyphenated (`std::fs`-`Operation`, `//!`-`Stil`).

German Rust writing keeps `Lint`, `Crate`, `Closure`, `Panic`, and `Fallback`
as English loans with German capitalization and gender; the catalogue follows
that practice, which the community translation listed in the
[Rust book translations appendix](https://doc.rust-lang.org/book/appendix-06-translation.html)
also reflects. `Modul`, `Funktion`, `Attribut`, `Variante`, `Makro`,
`Schwellenwert`, `Zweig`, `Testabdeckung`, and `Arbeitsverzeichnis` are
established native terms. `Capability` stays a loan because the native
candidates (`Fähigkeit`, `Berechtigung`) describe an ability or a permission,
not a capability handle.

The main hazards are these. `Attribut` serves for a Rust attribute; a Fluent
attribute never appears in a message, so translators' notes must qualify it
(`Fluent-Attribut`) when the distinction matters. `Panic` is kept distinct from
the everyday `Panik` so that the Rust mechanism is not read as an emotional
state; the verb is rendered as `eine Panic auslösen`. `Zweig` (a counted branch
of a predicate) must not be replaced by `Branch`, which German developers read
as a version-control branch; the abstract activity is `Verzweigung`.
`Grenzwert` renders the configured limit and `Schwellenwert` the bump
threshold, and the two must not be swapped. `Cooked-String`,
`Komplexitätsspitze`, and `innerer Dokumentationskommentar` are coinages with
no established German rendering: the first keeps the English qualifier because
German has no term for a string literal with escape processing, the second
renders the bump as a peak, and the third calques the Rust reference's term.

Worked example, from `common/locales/de/module_max_lines.ftl`:

```ftl
    .help = Teilen Sie { $module } in kleinere Module auf oder verringern Sie dessen Zuständigkeiten.
```

| title                | preferred                         | allowed                      | forbidden                                                |
| -------------------- | --------------------------------- | ---------------------------- | -------------------------------------------------------- |
| lint                 | `Lint`                            | `Lint-Regel`                 | —                                                        |
| crate                | `Crate`                           | —                            | `Kiste`; `Paket` (a Cargo package is a distinct concept) |
| module               | `Modul`                           | —                            | —                                                        |
| function             | `Funktion`                        | —                            | —                                                        |
| helper function      | `Hilfsfunktion`                   | —                            | —                                                        |
| closure              | `Closure`                         | —                            | `Abschluss`                                              |
| macro expansion      | `Makroexpansion`                  | `Makro-Expansion`            | —                                                        |
| attribute            | `Attribut`                        | —                            | —                                                        |
| outer attribute      | `äußeres Attribut`                | —                            | —                                                        |
| doc comment          | `Dokumentationskommentar`         | `Doc-Kommentar`              | —                                                        |
| inner doc comment    | `innerer Dokumentationskommentar` | —                            | —                                                        |
| test                 | `Test`                            | —                            | —                                                        |
| panic                | `Panic`                           | `eine Panic auslösen` (verb) | `Panik`                                                  |
| variant              | `Variante`                        | —                            | —                                                        |
| fallback             | `Fallback`                        | `Ersatz` (in compounds)      | —                                                        |
| cooked string        | `Cooked-String`                   | —                            | `gekochte Zeichenkette`                                  |
| escaped newline      | `maskierter Zeilenumbruch`        | —                            | —                                                        |
| cognitive complexity | `kognitive Komplexität`           | —                            | —                                                        |
| complexity bump      | `Komplexitätsspitze`              | —                            | —                                                        |
| threshold            | `Schwellenwert`                   | —                            | `Grenzwert` (reserved for the limit)                     |
| branch               | `Zweig`                           | `Verzweigung` (the activity) | `Branch`                                                 |
| test coverage        | `Testabdeckung`                   | —                            | —                                                        |
| working directory    | `Arbeitsverzeichnis`              | —                            | `Arbeitsordner`                                          |
| capability           | `Capability`                      | —                            | `Fähigkeit`                                              |
| diagnostic           | `Diagnose`                        | `Diagnosemeldung`            | —                                                        |

_Table 11: German terminology._

### Greek (`el`)

Greek diagnostics address the reader with the polite plural (`εσείς`) and
`.help` uses the plural imperative (`Εξαγάγετε`, `Διαιρέστε`, `Μετακινήστε`),
following the convention the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for Greek software. Conditions are stated in the passive or as plain
declaratives (`Εντοπίστηκαν`, `δεν προσμετράται`), which avoids gendering the
reader. The catalogue uses monotonic orthography, `«…»` quotation marks, and
the ano teleia (`·`) where the source uses a semicolon, because `;` is the
Greek question mark.

`lint` and `crate` stay in Latin script as indeclinable neuter loans (`το lint`,
`τα crate`), as do all identifiers; the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html)
show the same habit of leaving `crate` untranslated. Other terms use the
established Greek computing vocabulary: `συνάρτηση`, `μακροεντολή`,
`χαρακτηριστικό`, `σχόλιο τεκμηρίωσης`, `παραλλαγή`, `κλάδος`, and
`κατάλογος εργασίας`. `module` is rendered `λειτουργική μονάδα`, with the
Latin-script loan allowed.

Greek marks case on articles and nouns, so every placeable is given a Greek
head noun with its article (`την έκφραση «{ $name }»`,
`της τιμής { $receiver }`) and English fragments sit in `«…»`. Function names
such as `expect` take the feminine article by agreement with `συνάρτηση`.
`closure` is rendered `κλειστότητα`; `κλείσιμο` is avoided because it reads as
the act of closing. `χαρακτηριστικό` always means a Rust attribute, and
`δοκιμή` is always the noun. `cooked string` (`επεξεργασμένη συμβολοσειρά`),
`complexity bump` (`εξόγκωμα πολυπλοκότητας`), `escaped newline`
(`αλλαγή γραμμής με διαφυγή`), and `capability` (`δυνατότητα`) are coinages;
`δικαίωμα` is avoided for `capability` because it means a permission.

Worked example, from `common/locales/el/module_max_lines.ftl`:

```ftl
    .help = Διαιρέστε τη λειτουργική μονάδα { $module } σε μικρότερες ή μειώστε τις αρμοδιότητές της.
```

| title                | preferred                      | allowed                                                              | forbidden                                  |
| -------------------- | ------------------------------ | -------------------------------------------------------------------- | ------------------------------------------ |
| lint                 | `lint`                         | —                                                                    | —                                          |
| crate                | `crate`                        | —                                                                    | `πακέτο` (means a Cargo package)           |
| module               | `λειτουργική μονάδα`           | `module`                                                             | —                                          |
| function             | `συνάρτηση`                    | —                                                                    | —                                          |
| helper function      | `βοηθητική συνάρτηση`          | —                                                                    | —                                          |
| closure              | `κλειστότητα`                  | `closure`                                                            | `κλείσιμο`                                 |
| macro expansion      | `ανάπτυξη μακροεντολής`        | —                                                                    | —                                          |
| attribute            | `χαρακτηριστικό`               | —                                                                    | `ιδιότητα`                                 |
| outer attribute      | `εξωτερικό χαρακτηριστικό`     | —                                                                    | —                                          |
| doc comment          | `σχόλιο τεκμηρίωσης`           | —                                                                    | —                                          |
| inner doc comment    | `εσωτερικό σχόλιο τεκμηρίωσης` | —                                                                    | —                                          |
| test                 | `δοκιμή`                       | `τεστ` (informal)                                                    | —                                          |
| panic                | `πανικός`                      | —                                                                    | —                                          |
| variant              | `παραλλαγή`                    | —                                                                    | —                                          |
| fallback             | `εναλλακτική λύση`             | `εναλλακτικός` (adjective, as in `εναλλακτικά διαγνωστικά μηνύματα`) | —                                          |
| cooked string        | `επεξεργασμένη συμβολοσειρά`   | —                                                                    | `ανεπεξέργαστη συμβολοσειρά`               |
| escaped newline      | `αλλαγή γραμμής με διαφυγή`    | —                                                                    | —                                          |
| cognitive complexity | `γνωστική πολυπλοκότητα`       | —                                                                    | —                                          |
| complexity bump      | `εξόγκωμα πολυπλοκότητας`      | —                                                                    | —                                          |
| threshold            | `κατώφλι`                      | —                                                                    | `όριο` (reserved for the configured limit) |
| branch               | `κλάδος`                       | —                                                                    | —                                          |
| test coverage        | `κάλυψη δοκιμών`               | —                                                                    | —                                          |
| working directory    | `κατάλογος εργασίας`           | —                                                                    | `φάκελος`                                  |
| capability           | `δυνατότητα`                   | —                                                                    | `δικαίωμα`                                 |
| diagnostic           | `διαγνωστικό μήνυμα`           | `διαγνωστικό`                                                        | —                                          |

_Table 12: Greek terminology._

### English, United Kingdom (`en-GB`)

`en-GB` is the source locale, so its vocabulary is the source terminology in
Tables 1 to 6 and this section adds only register notes. The catalogue is
written in British English with Oxford spelling: "-ize" and "-yse" endings
("recognized"), "-our" ("behaviour"), and "-ogue". It is also the fallback
locale, so its wording is what a reader sees when another locale lacks a
message.

Instructions in `.help` text use the bare imperative ("Split the function into
smaller functions"), and notes are declarative. The catalogue does not address
the reader as "you", with one exception: `no_std_fs_operations.help` refers to
"your APIs". New source messages should follow the majority pattern.

Because every other catalogue is translated from this one, a change to the
wording of an `en-GB` message is a change to the source, and each translation
of that message needs review.

### English, United States (`en-US`)

The `en-US` catalogue differs from the source only where American spelling
differs from Oxford spelling. Oxford spelling already uses "-ize", so
"recognized" is the same in both; the one rendered word that changes is
"behaviour", which `en-US` spells `behavior` in
`public_fn_must_have_docs.help`. Register, punctuation, and terminology are
otherwise identical to `en-GB`, and the source tables apply unchanged.

The catalogue is kept as a complete copy, not a set of overrides, so that
`en-US` never depends on per-message fallback for its wording. When a source
message changes, apply the same change here and re-check it for words whose
spelling differs, including "-our", "-re", and "-ogue" endings and doubled
consonants such as "labelled".

`en-US` is the one translated locale whose text may equal the source text, and
the parity tests exempt English variants from the check that a translation is
not a copy.

Worked example, from `common/locales/en-US/public_fn_must_have_docs.ftl`:

```ftl
    .help = Add a doc comment describing the behavior of { $function }.
```

| title      | preferred    | allowed | forbidden    |
| ---------- | ------------ | ------- | ------------ |
| behaviour  | `behavior`   | —       | `behaviour`  |
| recognized | `recognized` | —       | `recognised` |

_Table 13: English (United States) terminology._

### Spanish, Latin America (`es-419`)

The `es-419` catalogue is written in neutral Latin American Spanish and differs
from `es-ES` in address, tense, vocabulary and punctuation. It addresses the
reader with the formal `usted`, the professional default that reads naturally
across the region, so `.help` text uses the third-person imperative (`Extraiga`,
`Divida`, `Mueva`, `Agregue`) and possessives use `su`/`sus` (`sus API`).
`vosotros` forms never appear, and `tú` forms are not mixed in. Completed
detections use the preterite (`Se detectaron`), where `es-ES` uses the present
perfect. English-style double quotation marks (`“{ $name }”`) enclose quoted
fragments, the prevailing practice in Latin American technical writing, where
`es-ES` uses angle quotation marks. Code keeps ASCII backticks. No sentence
requires gender agreement with the reader; `quienes llaman` stands in for a
gendered `los llamadores`.

`lint` and `crate` stay as masculine English loans (`los crates`, `los lints`),
as they do in the community Spanish translation of _The Rust Programming
Language_ listed in the
[book's translation appendix](https://doc.rust-lang.org/book/appendix-06-translation.html).
Regional vocabulary is used where it differs from Spain: `sistema de archivos`
(not `sistema de ficheros`), `agregar` (not `añadir`), `reemplazar` (not
`sustituir`), `manejar` (not `gestionar`), `conteo` (not `recuento`) and
`comenzar` (not `empezar`). Terms tied to one country are avoided.

Hazards: the crate name `camino` is also the ordinary Spanish word for a path
or way, so the catalogue writes it in backticks and uses `rutas` for paths.
`pánico` carries the everyday sense of alarm; the catalogue pairs it with a
code identifier or the fixed phrase `entrar en pánico`. `atributo` names both a
Rust attribute and a Fluent attribute; catalogue messages mean only the former.
`rama` (a counted branch) stays distinct from `ramificación` (branching in
general). `respaldo` also means a data backup in the region, so `fallback` is
always the full phrase `alternativa de respaldo` or the modifier `de respaldo`
after `diagnósticos`. `cooked string` (`cadena interpretada`), `complexity bump`
(`bache de complejidad`), `inner doc comment` and `capability` (`capacidad`)
are coinages or calques without settled Spanish usage in Rust writing.

Worked example, from `common/locales/es-419/module_max_lines.ftl`:

```ftl
module_max_lines = El módulo { $module } abarca { $lines } líneas y supera el límite permitido de { $limit }.
```

| title                | preferred                             | allowed                                   | forbidden                               |
| -------------------- | ------------------------------------- | ----------------------------------------- | --------------------------------------- |
| lint                 | `lint`                                | `regla de lint`                           | —                                       |
| crate                | `crate`                               | —                                         | `cajón`; `paquete`                      |
| module               | `módulo`                              | —                                         | —                                       |
| function             | `función`                             | —                                         | `método` (for a free function)          |
| helper function      | `función auxiliar`                    | —                                         | —                                       |
| closure              | `clausura`                            | `closure`                                 | `cierre` (reads as closing or shutdown) |
| macro expansion      | `expansión de macros`                 | —                                         | —                                       |
| attribute            | `atributo`                            | —                                         | `propiedad`                             |
| outer attribute      | `atributo externo`                    | —                                         | —                                       |
| doc comment          | `comentario de documentación`         | —                                         | `comentario` (unqualified)              |
| inner doc comment    | `comentario de documentación interno` | —                                         | —                                       |
| test                 | `prueba`                              | —                                         | `ensayo`                                |
| panic                | `pánico`                              | `entrar en pánico` (verb)                 | —                                       |
| variant              | `variante`                            | —                                         | `variación`                             |
| fallback             | `alternativa de respaldo`             | `de respaldo` (modifier)                  | `respaldo` (alone; reads as a backup)   |
| cooked string        | `cadena interpretada`                 | —                                         | `cadena cocinada`                       |
| escaped newline      | `salto de línea escapado`             | —                                         | —                                       |
| cognitive complexity | `complejidad cognitiva`               | —                                         | `complejidad ciclomática`               |
| complexity bump      | `bache de complejidad`                | —                                         | —                                       |
| threshold            | `umbral`                              | —                                         | `límite` (reserved for the limit)       |
| branch               | `rama`                                | `ramificación` (for branching in general) | —                                       |
| test coverage        | `cobertura de pruebas`                | —                                         | —                                       |
| working directory    | `directorio de trabajo`               | —                                         | `carpeta de trabajo`                    |
| capability           | `capacidad`                           | —                                         | `funcionalidad`                         |
| diagnostic           | `diagnóstico`                         | —                                         | —                                       |

_Table 14: Spanish (Latin America) terminology._

### Spanish, Spain (`es-ES`)

The `es-ES` catalogue is written for readers in Spain and differs from `es-419`
in address, tense, vocabulary and punctuation. It addresses the reader with the
informal `tú`, the form software published for Spain commonly adopts, and
`.help` text uses the singular imperative (`Extrae`, `Divide`, `Mueve`,
`Añade`); `vosotros` forms never appear because each diagnostic addresses one
reader. Completed detections use the present perfect (`Se han detectado`), where
`es-419` uses the preterite. Primary messages and notes otherwise state the
condition impersonally. Angle quotation marks (`«{ $name }»`) enclose quoted
fragments, as Spanish orthography recommends, and code keeps ASCII backticks.
Opening `¿` and `¡` would be required for questions and exclamations, but the
catalogue contains neither. No sentence requires gender agreement with the
reader; `quienes llaman` stands in for a gendered `los llamadores`.

`lint` and `crate` stay as masculine English loans (`los crates`, `los lints`),
as they do in the community Spanish translation of _The Rust Programming
Language_ listed in the
[book's translation appendix](https://doc.rust-lang.org/book/appendix-06-translation.html).
`API` is invariable in the plural (`tus API`). Peninsular vocabulary is used
where it differs: `sistema de ficheros` (not `sistema de archivos`), `añadir`
(not `agregar`), `sustituir` (not `reemplazar`), `gestionar` (not `manejar`) and
`recuento` (not `conteo`).

Hazards: the crate name `camino` is also the ordinary Spanish word for a path
or way, so the catalogue writes it in backticks and uses `rutas` for paths.
`pánico` carries the everyday sense of alarm; the catalogue pairs it with a
code identifier or the fixed phrase `entrar en pánico`. `atributo` names both a
Rust attribute and a Fluent attribute; catalogue messages mean only the former.
`rama` (a counted branch) stays distinct from `ramificación` (branching in
general). `prueba` is the noun for a test; `test` is tolerated in speech but
not used in the catalogue. `cooked string` (`cadena interpretada`),
`complexity bump` (`bache de complejidad`), `inner doc comment` and `capability`
(`capacidad`) are coinages or calques without settled Spanish usage in Rust
writing.

Worked example, from `common/locales/es-ES/module_max_lines.ftl`:

```ftl
module_max_lines = El módulo { $module } abarca { $lines } líneas y supera el límite permitido de { $limit }.
```

| title                | preferred                             | allowed                                   | forbidden                               |
| -------------------- | ------------------------------------- | ----------------------------------------- | --------------------------------------- |
| lint                 | `lint`                                | `regla de lint`                           | —                                       |
| crate                | `crate`                               | —                                         | `cajón`; `paquete`                      |
| module               | `módulo`                              | —                                         | —                                       |
| function             | `función`                             | —                                         | `método` (for a free function)          |
| helper function      | `función auxiliar`                    | —                                         | —                                       |
| closure              | `clausura`                            | `closure`                                 | `cierre` (reads as closing or shutdown) |
| macro expansion      | `expansión de macros`                 | —                                         | —                                       |
| attribute            | `atributo`                            | —                                         | `propiedad`                             |
| outer attribute      | `atributo externo`                    | —                                         | —                                       |
| doc comment          | `comentario de documentación`         | —                                         | `comentario` (unqualified)              |
| inner doc comment    | `comentario de documentación interno` | —                                         | —                                       |
| test                 | `prueba`                              | —                                         | `ensayo`                                |
| panic                | `pánico`                              | `entrar en pánico` (verb)                 | —                                       |
| variant              | `variante`                            | —                                         | `variación`                             |
| fallback             | `alternativa de respaldo`             | `de respaldo` (modifier)                  | `copia de seguridad`                    |
| cooked string        | `cadena interpretada`                 | —                                         | `cadena cocinada`                       |
| escaped newline      | `salto de línea escapado`             | —                                         | —                                       |
| cognitive complexity | `complejidad cognitiva`               | —                                         | `complejidad ciclomática`               |
| complexity bump      | `bache de complejidad`                | —                                         | —                                       |
| threshold            | `umbral`                              | —                                         | `límite` (reserved for the limit)       |
| branch               | `rama`                                | `ramificación` (for branching in general) | —                                       |
| test coverage        | `cobertura de pruebas`                | —                                         | —                                       |
| working directory    | `directorio de trabajo`               | —                                         | `carpeta de trabajo`                    |
| capability           | `capacidad`                           | —                                         | `funcionalidad`                         |
| diagnostic           | `diagnóstico`                         | —                                         | —                                       |

_Table 15: Spanish (Spain) terminology._

### Persian (`fa`)

Persian diagnostics address the reader with the polite plural `شما` and its
imperative (`استخراج کنید`, `تقسیم کنید`, `اضافه کنید`), which is the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
establish for software; the familiar `تو` form is never used. Primary messages
and notes state the condition declaratively. Persian has no grammatical gender,
so no inclusivity device is needed. Nouns stay singular after a numeral, so the
`one` and `other` variants of each plural select carry identical wording. The
catalogue uses the zero-width non-joiner (U+200C) wherever standard Persian
orthography requires it (`می‌شود`, `تابع‌های`), Persian punctuation (`،` and
`؛`), and guillemets (`«»`) as quotation marks. Directionality mechanics are
owned by the translator's guide (`translators-guide.md`, section 8).

Native terms are established for `تابع`, `شاخه`, `آستانه`, `آزمون`, and
`پیچیدگی شناختی`. `لینت`, `کریت`, `ماژول`, `کلوژر`, `ماکرو`, `کامنت`, `پنیک`,
and `هندل` are loans transliterated into Persian script, as is common in
Persian developer prose. Identifiers and code stay in Latin script.

`صفت` renders a Rust attribute; `ویژگی` is avoided because it also means a
feature, a property, and (in Rust writing) a trait. `کامنت مستندات` is
preferred over `نظر`, which names a user comment or opinion. `جایگزین` renders
`fallback` and must not be read as a general replacement of one item by another
outside that context. `قابلیت` renders `capability` and also means a feature in
everyday product prose, so the catalogue always pairs it with `مبتنی بر` or
`سطح`. `رشتهٔ پردازش‌شده` for `cooked string`, `خط جدید گریزدار` for
`escaped newline`, `برآمدگی پیچیدگی` for `complexity bump`, and
`کامنت مستندات درونی` for `inner doc comment` are coinages with no established
Persian rendering.

Worked example, from `common/locales/fa/module_max_lines.ftl`:

```ftl
module_max_lines = ماژول { $module } شامل { $lines } خط است و از حد مجاز { $limit } فراتر می‌رود.
```

| title                | preferred             | allowed          | forbidden  |
| -------------------- | --------------------- | ---------------- | ---------- |
| lint                 | `لینت`                | `lint`           | —          |
| crate                | `کریت`                | `crate`          | `بسته`     |
| module               | `ماژول`               | `پیمانه`         | —          |
| function             | `تابع`                | —                | —          |
| helper function      | `تابع کمکی`           | —                | —          |
| closure              | `کلوژر`               | `بستار`          | —          |
| macro expansion      | `بسط ماکرو`           | —                | —          |
| attribute            | `صفت`                 | —                | `ویژگی`    |
| outer attribute      | `صفت بیرونی`          | —                | —          |
| doc comment          | `کامنت مستندات`       | `توضیح مستندات`  | `نظر`      |
| inner doc comment    | `کامنت مستندات درونی` | —                | —          |
| test                 | `آزمون`               | `تست`            | —          |
| panic                | `پنیک`                | `panic`          | `وحشت`     |
| variant              | `گونه`                | —                | `متغیر`    |
| fallback             | `جایگزین`             | —                | `پشتیبان`  |
| cooked string        | `رشتهٔ پردازش‌شده`      | —                | `رشتهٔ خام` |
| escaped newline      | `خط جدید گریزدار`     | —                | —          |
| cognitive complexity | `پیچیدگی شناختی`      | —                | —          |
| complexity bump      | `برآمدگی پیچیدگی`     | —                | —          |
| threshold            | `آستانه`              | —                | `حد`       |
| branch               | `شاخه`                | —                | —          |
| test coverage        | `پوشش آزمون`          | `پوشش تست`       | —          |
| working directory    | `پوشهٔ کاری`           | `دایرکتوری کاری` | —          |
| capability           | `قابلیت`              | —                | `مجوز`     |
| diagnostic           | `پیام تشخیصی`         | —                | —          |

_Table 16: Persian terminology._

### Finnish (`fi`)

Finnish diagnostics address the reader with the second-person singular (`sinä`)
imperative, the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
use for Finnish software; the pronoun itself is never written, and the polite
plural `te` never appears. Messages that demand a change and every `.help`
attribute use that imperative (`Siirrä …`, `Jaa …`), while condition reports
use the passive or a plain declarative (`Havaittiin …`, `… ei lasketa`).
Finnish has no grammatical gender, so no inclusivity device is needed. Both
quotation marks are the right-leaning double mark.

Finnish inflects nouns by case, and a frozen identifier must never carry a case
ending. The catalogue therefore puts the ending on a head noun placed before
the placeable (`moduulin { $module }`, `funktion { $function }`,
`kohteen { $receiver }`) or attaches it to a hyphenated head
(`expect`-`metodia`, `concat!()`-`lausekkeella`). English fragments in
quotation marks are likewise introduced by the neutral head noun `kohde`. The
catalogue keeps `crate` and `cooked` as loans and `lint` only inside the
compound `lint-sääntö`; `moduuli`, `funktio`, `apufunktio`, `sulkeuma`,
`attribuutti`, `variantti`, `kynnysarvo`, `haara`, `testikattavuus`, and
`työhakemisto` are established native or adapted terms.

The main hazards are these. Numerals other than one govern the partitive
singular (`{ $branches } haaraa`, `{ $lines } riviä`), whereas a numeral inside
a case-marked phrase leaves the noun in that case for every number
(`enintään { $limit } haaraan`, `{ $lines } rivin pituinen`); the `one` and
`other` variants of those two messages are consequently identical, but both are
listed because the engine defines both categories. `haara` renders a counted
branch and must not be replaced by `haarauma`, which reads as a version-control
branch; the activity is `haarautuminen`. `paniikki` is the adapted Rust term
but coincides with the everyday word, so it always appears with `aiheuttaa`.
`raja` renders the configured limit and `kynnysarvo` the bump threshold.
`cooked-merkkijono`, `kompleksisuushuippu`, `sisempi dokumentaatiokommentti`,
`escape-merkitty rivinvaihto`, and `kyvykkyys` (for capability) are coinages
with no established Finnish rendering.

Worked example, from `common/locales/fi/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Moduuli { $module } on { $lines } rivin pituinen ja ylittää sallitun rajan { $limit }.
       *[other] Moduuli { $module } on { $lines } rivin pituinen ja ylittää sallitun rajan { $limit }.
    }
```

| title                | preferred                        | allowed                        | forbidden                                                     |
| -------------------- | -------------------------------- | ------------------------------ | ------------------------------------------------------------- |
| lint                 | `lint-sääntö`                    | `lint` (in compounds)          | —                                                             |
| crate                | `crate`                          | —                              | `laatikko`; `paketti` (a Cargo package is a distinct concept) |
| module               | `moduuli`                        | —                              | —                                                             |
| function             | `funktio`                        | —                              | —                                                             |
| helper function      | `apufunktio`                     | —                              | —                                                             |
| closure              | `sulkeuma`                       | `closure`                      | —                                                             |
| macro expansion      | `makron laajennus`               | —                              | —                                                             |
| attribute            | `attribuutti`                    | —                              | —                                                             |
| outer attribute      | `ulompi attribuutti`             | —                              | —                                                             |
| doc comment          | `dokumentaatiokommentti`         | —                              | —                                                             |
| inner doc comment    | `sisempi dokumentaatiokommentti` | —                              | —                                                             |
| test                 | `testi`                          | —                              | —                                                             |
| panic                | `paniikki`                       | `aiheuttaa paniikin` (verb)    | —                                                             |
| variant              | `variantti`                      | —                              | —                                                             |
| fallback             | `varavaihtoehto`                 | `vara-` (in compounds)         | —                                                             |
| cooked string        | `cooked-merkkijono`              | —                              | `keitetty merkkijono`                                         |
| escaped newline      | `escape-merkitty rivinvaihto`    | —                              | —                                                             |
| cognitive complexity | `kognitiivinen kompleksisuus`    | —                              | —                                                             |
| complexity bump      | `kompleksisuushuippu`            | —                              | —                                                             |
| threshold            | `kynnysarvo`                     | —                              | `raja` (reserved for the limit)                               |
| branch               | `haara`                          | `haarautuminen` (the activity) | `haarauma`                                                    |
| test coverage        | `testikattavuus`                 | —                              | —                                                             |
| working directory    | `työhakemisto`                   | —                              | `työkansio`                                                   |
| capability           | `kyvykkyys`                      | —                              | `oikeus`                                                      |
| diagnostic           | `diagnostiikka`                  | `diagnostiikkaviesti`          | —                                                             |

_Table 17: Finnish terminology._

### French (`fr`)

French diagnostics address the reader with the formal `vous`, never `tu`, which
follows the convention of the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
for professional software. Primary messages and notes state the condition
impersonally in the present tense (`Le module … dépasse la limite`), and
`.help` text instructs with the second-person plural imperative (`Extrayez`,
`Scindez`, `Déplacez`). Span labels and suggestions are noun phrases or bare
infinitives (`remplacer par…`) and take no terminal full stop. The catalogue
avoids gendering the reader: no sentence needs an adjective or participle that
agrees with the person addressed. French typography applies: guillemets with
no-break spaces (U+00A0) enclose quoted fragments, a no-break space precedes
`:` and `;`, and the typographic apostrophe `’` marks elision. Code keeps ASCII
backticks.

`lint` and `crate` stay as English loans, as they do in the community French
translation of _The Rust Programming Language_ listed in the
[book's translation appendix](https://doc.rust-lang.org/book/appendix-06-translation.html);
the catalogue treats `lint` as masculine and `crate` as feminine
(`les crates en aval`). Everything else in the table has an established native
term: `fermeture`, `attribut`, `commentaire de documentation`, `seuil`,
`branche`, `répertoire de travail`. `API` is invariable in the plural
(`vos API`).

Three hazards need care. `panic` is rendered with the native `panique` and the
verb `paniquer`, which also carry the everyday sense of alarm; the catalogue
always pairs them with a code identifier so the Rust sense is plain. `attribut`
names both a Rust attribute and a Fluent attribute; catalogue messages only
ever mean the former, and documentation qualifies the latter as
`attribut Fluent`. `branche` (a counted branch of a predicate) must stay
distinct from `embranchements` (branching in general) and from a
version-control branch. The renderings of `cooked string`
(`chaîne interprétée`), `complexity bump` (`bosse de complexité`),
`inner doc comment` and `capability` (`capacité`) are coinages or calques with
no settled French usage in Rust writing. English fragments such as `{ $name }`
and `{ $subject }` sit inside guillemets so that they need no article or
agreement; the catalogue also writes `camino` in backticks because the crate
name follows `chemins` (paths) directly.

Worked example, from `common/locales/fr/module_max_lines.ftl`:

```ftl
module_max_lines = Le module { $module } s’étend sur { $lines } lignes et dépasse la limite autorisée de { $limit }.
```

| title                | preferred                              | allowed                                     | forbidden                         |
| -------------------- | -------------------------------------- | ------------------------------------------- | --------------------------------- |
| lint                 | `lint`                                 | `règle de lint`                             | —                                 |
| crate                | `crate`                                | —                                           | `caisse`; `paquet`                |
| module               | `module`                               | —                                           | —                                 |
| function             | `fonction`                             | —                                           | `méthode` (for a free function)   |
| helper function      | `fonction auxiliaire`                  | `fonction utilitaire`                       | —                                 |
| closure              | `fermeture`                            | `closure`                                   | `clôture`                         |
| macro expansion      | `expansion de macros`                  | `expansion de macro`                        | —                                 |
| attribute            | `attribut`                             | —                                           | `propriété`                       |
| outer attribute      | `attribut externe`                     | —                                           | `attribut extérieur`              |
| doc comment          | `commentaire de documentation`         | —                                           | `commentaire` (unqualified)       |
| inner doc comment    | `commentaire de documentation interne` | —                                           | —                                 |
| test                 | `test`                                 | —                                           | `essai`                           |
| panic                | `panique`                              | `paniquer` (verb)                           | `plantage`                        |
| variant              | `variante`                             | —                                           | `variation`                       |
| fallback             | `solution de repli`                    | `de repli` (modifier)                       | —                                 |
| cooked string        | `chaîne interprétée`                   | `chaîne non brute`                          | `chaîne cuite`                    |
| escaped newline      | `saut de ligne échappé`                | —                                           | —                                 |
| cognitive complexity | `complexité cognitive`                 | —                                           | `complexité cyclomatique`         |
| complexity bump      | `bosse de complexité`                  | —                                           | —                                 |
| threshold            | `seuil`                                | —                                           | `limite` (reserved for the limit) |
| branch               | `branche`                              | `embranchements` (for branching in general) | —                                 |
| test coverage        | `couverture de tests`                  | `couverture des tests`                      | —                                 |
| working directory    | `répertoire de travail`                | —                                           | `dossier de travail`              |
| capability           | `capacité`                             | —                                           | `fonctionnalité`                  |
| diagnostic           | `diagnostic`                           | —                                           | `diagnostique` (as a noun)        |

_Table 18: French terminology._

### Scottish Gaelic (`gd`)

The Scottish Gaelic catalogue shipped before this glossary existed, and this
section records the vocabulary it uses. Instructions in `.help` text use the
singular imperative (`Gluais`, `Roinn`, `Sgar`, `Na gairm`); one note
(`no_std_fs_operations.note`) uses the plural `gabhaibh`. Notes are declarative
statements with `Tha` or `Chan e`. The catalogue does not use the polite plural
`sibh` consistently, and the address form is an open question for review.

Gaelic lenites and inflects nouns after the article, prepositions, and
numerals, so a term appears in several written forms: `mòideal` is also written
`mhòideal`, and `deuchainn` becomes `dheuchainn`. The table gives the citation
form. Interpolated identifiers are never lenited. The plural engine
distinguishes `one`, `two`, `few`, and `other` for Gaelic, and the catalogue
lists all four in each plural select; the `branch_phrase` helper described in
the [translator's guide](translators-guide.md) renders branch counts for this
locale.

The catalogue keeps `lint`, `macro`, `doc`, and `bump` as English loans, and
adapts "crate" as `criot`. Native terms cover the rest of the vocabulary.

Several points are open questions for a Gaelic-speaking reviewer:

- `common-lint-count.fallback-note` is not translated, so it renders the
  `en-GB` text. The parity tests record this as a known gap.
- `eòlas àrainneachd` renders "ambient working directory" in
  `no_std_fs_operations.note`; `eòlas` usually means knowledge, and a word for
  a directory may have been intended.
- `sreang bruich` is a literal calque of "cooked string".
- `làimhseachadh` (handling) renders "fallback" in `no_unwrap_or_else_panic`.
- `string_continuation_style.ftl` uses the ASCII apostrophe where every other
  file uses the typographic one.

Worked example, from `common/locales/gd/module_max_lines.ftl`:

```ftl
    .help = Roinn { $module } no lughdaich an luchd dleastanais.
```

| title                | preferred                    | allowed                 | forbidden |
| -------------------- | ---------------------------- | ----------------------- | --------- |
| lint                 | `lint`                       | —                       | —         |
| crate                | `criot`                      | `criotan` (plural)      | —         |
| module               | `mòideal`                    | —                       | —         |
| function             | `gnìomh`                     | —                       | —         |
| helper function      | `gnìomh-taice`               | `còd-taice`             | —         |
| closure              | `dùnadh`                     | —                       | —         |
| macro expansion      | `leudachadh macro`           | —                       | —         |
| attribute            | `buadh`                      | —                       | —         |
| outer attribute      | `buadh taobh a-muigh`        | —                       | —         |
| doc comment          | `beachd doc`                 | —                       | —         |
| inner doc comment    | `beachd a-staigh`            | —                       | —         |
| test                 | `deuchainn`                  | `deuchainnean` (plural) | —         |
| panic                | `clisgeadh`                  | —                       | —         |
| variant              | `caochladh`                  | `cùis` (for `None`)     | —         |
| fallback             | `làimhseachadh`              | —                       | —         |
| cooked string        | `sreang bruich`              | —                       | —         |
| escaped newline      | `loidhne ùr theichte`        | —                       | —         |
| cognitive complexity | `iom-fhillteachd inntinneil` | —                       | —         |
| complexity bump      | `bump iom-fhillteachd`       | —                       | —         |
| threshold            | `stairsneach`                | —                       | —         |
| branch               | `meur`                       | `meuran` (plural)       | —         |
| test coverage        | `còmhdach dheuchainnean`     | —                       | —         |
| working directory    | `eòlas àrainneachd`          | —                       | —         |
| capability           | `comas`                      | `comasan` (plural)      | —         |
| diagnostic           | `breithneachadh`             | —                       | —         |

_Table 19: Scottish Gaelic terminology._

### Hebrew (`he`)

Hebrew diagnostics avoid addressing the reader directly. `.help` text uses the
impersonal construction `יש` with an infinitive (`יש לחלץ`, `יש לפצל`,
`יש להוסיף`), which is gender-neutral and standard in formal Hebrew technical
prose; the gendered singular imperative is never used. Primary messages and
notes state the condition declaratively (`קיימים`, `חייב להתחיל`). The
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
are the reference for register. Text is written without vowel points. Every
plural select lists `one`, `two`, `many`, and `other`: the catalogue writes the
singular and the dual in words (`שורה אחת`, `שתי שורות`) and uses the numeral
with the plural noun in the remaining categories. Directionality mechanics are
owned by the translator's guide (`translators-guide.md`, section 8).

Native terms are established for `פונקציה`, `מודול`, `ענף`, `סף`, `בדיקה`,
`הערת תיעוד`, and `ספריית עבודה`. `lint`, `crate`, and `panic` stay as English
loans in Latin script, as Hebrew developer prose commonly leaves them, and
`מאקרו` and `וריאנט` are transliterated loans. A Hebrew prefix letter placed
before a Latin identifier is joined to it with a hyphen (`ל-panic`, `ב-`…), and
identifiers and code stay in Latin script.

`תכונה` renders a Rust attribute but also means a property or feature in
everyday product prose, so the catalogue uses it only beside `חיצונית` or an
interpolated attribute. `בדיקה` renders a `test` and also means a check in
general; the catalogue keeps it for test items only. `ספרייה` means both a
directory and a library; `ספריית עבודה` is unambiguous, and the casual `תיקייה`
is avoided. `חלופה` renders `fallback` and must not become `גיבוי`, which means
a backup copy. `סגור` for `closure` is not settled in developer usage, and the
English loan remains an allowed alternative. `מחרוזת מעובדת` for
`cooked string`, `מעבר שורה עם תו בריחה` for `escaped newline`,
`גבשושית מורכבות` for `complexity bump`, `הערת תיעוד פנימית` for
`inner doc comment`, and `יכולת` for `capability` are coinages with no
established Hebrew rendering.

Worked example, from `common/locales/he/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] המודול { $module } משתרע על שורה אחת וחורג מהמגבלה המותרת { $limit }.
        [two] המודול { $module } משתרע על שתי שורות וחורג מהמגבלה המותרת { $limit }.
        [many] המודול { $module } משתרע על { $lines } שורות וחורג מהמגבלה המותרת { $limit }.
       *[other] המודול { $module } משתרע על { $lines } שורות וחורג מהמגבלה המותרת { $limit }.
    }
```

| title                | preferred               | allowed       | forbidden       |
| -------------------- | ----------------------- | ------------- | --------------- |
| lint                 | `lint`                  | `כלל lint`    | —               |
| crate                | `crate`                 | `קרייט`       | `חבילה`         |
| module               | `מודול`                 | —             | —               |
| function             | `פונקציה`               | —             | —               |
| helper function      | `פונקציית עזר`          | —             | —               |
| closure              | `סגור`                  | `closure`     | —               |
| macro expansion      | `הרחבת מאקרו`           | `פריסת מאקרו` | —               |
| attribute            | `תכונה`                 | `attribute`   | `מאפיין`        |
| outer attribute      | `תכונה חיצונית`         | —             | —               |
| doc comment          | `הערת תיעוד`            | —             | —               |
| inner doc comment    | `הערת תיעוד פנימית`     | —             | —               |
| test                 | `בדיקה`                 | `טסט`         | —               |
| panic                | `panic`                 | `פאניקה`      | —               |
| variant              | `וריאנט`                | —             | `משתנה`         |
| fallback             | `חלופה`                 | —             | `גיבוי`         |
| cooked string        | `מחרוזת מעובדת`         | —             | `מחרוזת גולמית` |
| escaped newline      | `מעבר שורה עם תו בריחה` | —             | —               |
| cognitive complexity | `מורכבות קוגניטיבית`    | —             | —               |
| complexity bump      | `גבשושית מורכבות`       | —             | —               |
| threshold            | `סף`                    | —             | `מגבלה`         |
| branch               | `ענף`                   | —             | —               |
| test coverage        | `כיסוי בדיקות`          | —             | —               |
| working directory    | `ספריית עבודה`          | —             | `תיקיית עבודה`  |
| capability           | `יכולת`                 | —             | `הרשאה`         |
| diagnostic           | `הודעת אבחון`           | `אבחון`       | —               |

_Table 20: Hebrew terminology._

### Hindi (`hi`)

<!-- markdownlint-disable MD013 MD060 -->

Hindi diagnostics use the honorific `आप` register with the polite imperative in
`-एँ`/`-ें` (`निकालें`, `जोड़ें`, `बाँटें`), which is the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
establish for software; the familiar `तुम` and intimate `तू` forms are never
used. Primary messages and notes state the condition declaratively (`है`,
`होनी चाहिए`). Sentences end with the danda (`।`), not a full stop.
Instructions are phrased without a gendered subject, so the reader's gender is
never assumed. Hindi's `one` plural category covers both zero and one.

Hindi technical prose keeps many English loans, transliterated into Devanagari:
`लिंट`, `क्रेट`, `मॉड्यूल`, `फ़ंक्शन`, `क्लोज़र`, `मैक्रो`, `एट्रिब्यूट`, `टेस्ट`, `पैनिक`,
`वैरिएंट`, `फ़ॉलबैक`, and `थ्रेशोल्ड`. Native terms are used where they are
established: `शाखा`, `टिप्पणी`, `संज्ञानात्मक जटिलता`, `कार्यशील निर्देशिका`, and
`निदान`. Identifiers and code stay in Latin script.

`सीमा` renders `limit`, so `threshold` is kept as the loan `थ्रेशोल्ड` to stop the
two settings merging. `एट्रिब्यूट` is preferred over `विशेषता` for a Rust
attribute, because `विशेषता` also means a feature or property. `टेस्ट` names the
test item; `परीक्षण` is acceptable but reads as the activity of testing. `शाखा`
is grammatically feminine and inflects after a numeral (`शाखा`, `शाखाएँ`, oblique
`शाखाओं`), which is why each plural variant is a whole sentence. `कुक्ड स्ट्रिंग` for
`cooked string`, `जटिलता उभार` for `complexity bump`, `आंतरिक दस्तावेज़ टिप्पणी` for
`inner doc comment`, and `क्षमता` for `capability` are coinages with no
established Hindi rendering.

Worked example, from `common/locales/hi/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] मॉड्यूल { $module } में { $lines } पंक्ति है, जो अनुमत सीमा { $limit } से अधिक है।
       *[other] मॉड्यूल { $module } में { $lines } पंक्तियाँ हैं, जो अनुमत सीमा { $limit } से अधिक हैं।
    }
```

| title                | preferred             | allowed      | forbidden  |
| -------------------- | --------------------- | ------------ | ---------- |
| lint                 | `लिंट`                 | `lint`       | —          |
| crate                | `क्रेट`                 | `crate`      | `पैकेज`      |
| module               | `मॉड्यूल`               | —            | —          |
| function             | `फ़ंक्शन`                | —            | —          |
| helper function      | `सहायक फ़ंक्शन`          | —            | —          |
| closure              | `क्लोज़र`               | —            | —          |
| macro expansion      | `मैक्रो विस्तार`         | —            | —          |
| attribute            | `एट्रिब्यूट`             | `विशेषता`     | `गुण`       |
| outer attribute      | `बाहरी एट्रिब्यूट`       | —            | —          |
| doc comment          | `दस्तावेज़ टिप्पणी`       | `डॉक टिप्पणी` | —          |
| inner doc comment    | `आंतरिक दस्तावेज़ टिप्पणी` | —            | —          |
| test                 | `टेस्ट`                 | `परीक्षण`     | —          |
| panic                | `पैनिक`                | —            | `घबराहट`   |
| variant              | `वैरिएंट`               | —            | `चर`       |
| fallback             | `फ़ॉलबैक`               | —            | —          |
| cooked string        | `कुक्ड स्ट्रिंग`           | —            | `रॉ स्ट्रिंग` |
| escaped newline      | `एस्केप की गई नई पंक्ति`  | —            | —          |
| cognitive complexity | `संज्ञानात्मक जटिलता`    | —            | —          |
| complexity bump      | `जटिलता उभार`         | —            | —          |
| threshold            | `थ्रेशोल्ड`              | —            | `सीमा`     |
| branch               | `शाखा`                | —            | —          |
| test coverage        | `टेस्ट कवरेज`            | —            | —          |
| working directory    | `कार्यशील निर्देशिका`    | —            | `फ़ोल्डर`    |
| capability           | `क्षमता`               | —            | `अनुमति`    |
| diagnostic           | `निदान`               | `निदान संदेश`  | —          |

_Table 21: Hindi terminology._

<!-- markdownlint-enable MD013 MD060 -->

### Hungarian (`hu`)

Hungarian diagnostics use the formal third-person address (`önözés`) without
the pronoun, so `.help` instructions take the polite imperative (`Emeljen ki`,
`Bontsa`, `Helyezze át`); this is the register the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for Hungarian software. Conditions are plain declarative statements.
Hungarian has no grammatical gender, so no inclusivity workaround is needed.
Quotation marks are `„…”`. Because the definite article depends on the first
sound of the following word, an article that precedes a placeable is written
`A(z)` or `a(z)`.

`lint` and `crate` stay as English loans and take Hungarian suffixes, with a
hyphen where the final letter is silent (`lintek`, `crate-ek`); the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html)
show the same preference for keeping `crate`. `modul`, `függvény`,
`segédfüggvény`, `makró`, `attribútum`, `variáns`, and `pánik` are established.
`closure` is rendered `lezárás`, with the English loan allowed.

A Hungarian noun stays singular after a numeral, so the `one` and `other`
variants of each counted sentence read alike; both are listed because the
plural engine defines both categories. Suffixes cannot be attached to an
English fragment or a code snippet, so each placeable is followed by a
Hungarian head noun that carries the case ending (`a(z) „{ $name }” kifejezést`,
`a(z) { $module } modult`). `branch` is `ág`; the unrelated `error path` is
rendered `hibaútvonal` so that it is not read as a branch. `cooked string`
(`feldolgozott karakterlánc`), `complexity bump` (`komplexitási bukkanó`), and
`capability` (`képesség`) are coinages; `jogosultság` is avoided for
`capability` because it means a permission. `escape-elt` is the customary
hyphenated loan participle. `attribútum` always means a Rust attribute, and
`teszt` is always the noun.

Worked example, from `common/locales/hu/module_max_lines.ftl`:

```ftl
module_max_lines = A(z) { $module } modul { $lines } sorból áll, ami meghaladja a megengedett { $limit } értéket.
```

| title                | preferred                        | allowed                                                 | forbidden                                    |
| -------------------- | -------------------------------- | ------------------------------------------------------- | -------------------------------------------- |
| lint                 | `lint`                           | —                                                       | —                                            |
| crate                | `crate`                          | `láda`                                                  | `csomag` (means a Cargo package)             |
| module               | `modul`                          | —                                                       | —                                            |
| function             | `függvény`                       | —                                                       | —                                            |
| helper function      | `segédfüggvény`                  | —                                                       | —                                            |
| closure              | `lezárás`                        | `closure`                                               | —                                            |
| macro expansion      | `makrókifejtés`                  | —                                                       | —                                            |
| attribute            | `attribútum`                     | —                                                       | `tulajdonság`                                |
| outer attribute      | `külső attribútum`               | —                                                       | —                                            |
| doc comment          | `dokumentációs megjegyzés`       | `dokumentációs komment` (informal)                      | —                                            |
| inner doc comment    | `belső dokumentációs megjegyzés` | —                                                       | —                                            |
| test                 | `teszt`                          | —                                                       | —                                            |
| panic                | `pánik`                          | —                                                       | —                                            |
| variant              | `variáns`                        | `változat`                                              | —                                            |
| fallback             | `tartalék megoldás`              | `tartalék` (attributive, as in `tartalék diagnosztika`) | —                                            |
| cooked string        | `feldolgozott karakterlánc`      | —                                                       | `nyers karakterlánc`                         |
| escaped newline      | `escape-elt sortörés`            | —                                                       | —                                            |
| cognitive complexity | `kognitív komplexitás`           | —                                                       | —                                            |
| complexity bump      | `komplexitási bukkanó`           | —                                                       | —                                            |
| threshold            | `küszöbérték`                    | —                                                       | `korlát` (reserved for the configured limit) |
| branch               | `ág`                             | —                                                       | —                                            |
| test coverage        | `tesztlefedettség`               | —                                                       | —                                            |
| working directory    | `munkakönyvtár`                  | —                                                       | `mappa`                                      |
| capability           | `képesség`                       | —                                                       | `jogosultság`                                |
| diagnostic           | `diagnosztika`                   | `diagnosztikai üzenet`                                  | —                                            |

_Table 22: Hungarian terminology._

### Indonesian (`id`)

Indonesian diagnostics use standard formal Indonesian (`bahasa baku`) in a
neutral register. The reader is not addressed with `Anda` or `kamu`; conditions
are plain statements, and instructions use the bare imperative verb (`Ekstrak`,
`Pindahkan`, `Tambahkan`), the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for user-interface instructions, without the softening particle
`-lah` or `silakan`. Indonesian has no grammatical gender. Span labels are
lower-case noun or verb phrases without a full stop. Latin punctuation is used,
with `“ ”` as the primary quotation marks, and English fragments supplied by
the lint are enclosed in them. Nouns are not marked for number after a numeral,
so counts are written inline (`{ $branches } cabang`, `{ $lines } baris`) and
no plural select is needed; reduplication (`modul-modul`) is used only where
plurality carries meaning without a numeral.

Indonesian developer writing keeps `lint`, `crate`, `closure`, `panic`,
`handle`, `path`, `slug`, and `literal` as loans, and the catalogue does the
same; identifiers are never translated. Standard Indonesian terms are used for
`modul`, `fungsi`, `atribut`, `makro`, `komentar dokumentasi`, `pengujian`,
`varian`, `cabang`, `ambang batas`, `direktori kerja`, `berkas`, and `galat`.

`pengujian` is the noun for a test, while `menguji` is the verb; `tes` is
acceptable but less formal. `cabang` renders branch and `percabangan` renders
branching. `cadangan` renders fallback; it also means backup, so the catalogue
never uses it for a copy of data. `kapabilitas` renders capability and is kept
distinct from `izin`, which means permission. `atribut luar` and
`komentar dokumentasi dalam` are literal renderings of Rust's outer and inner;
the second is a coinage. Other coinages with no established Indonesian
rendering: `string non-mentah` for cooked string (by contrast with
`string mentah`, raw string), `baris baru yang di-escape` for escaped newline,
and `tonjolan kompleksitas` for complexity bump.

Worked example, from `common/locales/id/module_max_lines.ftl`:

```ftl
module_max_lines = Modul { $module } terdiri atas { $lines } baris, melebihi batas { $limit } baris yang diizinkan.
```

| title                | preferred                    | allowed              | forbidden                                         |
| -------------------- | ---------------------------- | -------------------- | ------------------------------------------------- |
| lint                 | `lint`                       | —                    | —                                                 |
| crate                | `crate`                      | —                    | `paket` (a Cargo package is a different concept)  |
| module               | `modul`                      | —                    | —                                                 |
| function             | `fungsi`                     | —                    | —                                                 |
| helper function      | `fungsi pembantu`            | `fungsi bantu`       | —                                                 |
| closure              | `closure`                    | —                    | —                                                 |
| macro expansion      | `ekspansi makro`             | `perluasan makro`    | —                                                 |
| attribute            | `atribut`                    | —                    | —                                                 |
| outer attribute      | `atribut luar`               | —                    | —                                                 |
| doc comment          | `komentar dokumentasi`       | `komentar dok`       | —                                                 |
| inner doc comment    | `komentar dokumentasi dalam` | —                    | —                                                 |
| test                 | `pengujian`                  | `tes`                | —                                                 |
| panic                | `panic`                      | `panik`              | —                                                 |
| variant              | `varian`                     | —                    | `variabel` (means variable)                       |
| fallback             | `cadangan`                   | `fallback`           | —                                                 |
| cooked string        | `string non-mentah`          | `string biasa`       | `string mentah` (means raw string)                |
| escaped newline      | `baris baru yang di-escape`  | —                    | —                                                 |
| cognitive complexity | `kompleksitas kognitif`      | —                    | `kompleksitas siklomatik` (cyclomatic complexity) |
| complexity bump      | `tonjolan kompleksitas`      | —                    | —                                                 |
| threshold            | `ambang batas`               | `ambang`             | —                                                 |
| branch               | `cabang`                     | —                    | —                                                 |
| test coverage        | `cakupan pengujian`          | `cakupan tes`        | —                                                 |
| working directory    | `direktori kerja`            | `direktori saat ini` | `folder kerja`                                    |
| capability           | `kapabilitas`                | —                    | `izin` (permission)                               |
| diagnostic           | `diagnostik`                 | `pesan diagnostik`   | —                                                 |

_Table 23: Indonesian terminology._

### Italian (`it`)

Italian diagnostics avoid addressing the reader directly, which sidesteps the
choice between `tu` and `Lei`. Primary messages and notes state the condition
impersonally in the present tense (`Il modulo … supera il limite`), and `.help`
text instructs with the impersonal infinitive (`Estrarre`, `Suddividere`,
`Spostare`), the form Italian technical documentation conventionally uses for
procedures. Possessives that would point at the reader become `le proprie`
(`le proprie API`). Span labels and suggestions are noun phrases or infinitives
without a terminal full stop. Low guillemets without inner spaces
(`«{ $name }»`) enclose quoted fragments, the typographic apostrophe `’` marks
elision, and code keeps ASCII backticks. No sentence requires gender agreement
with the reader.

Italian developer prose keeps many English loans, and the catalogue follows
that habit for `lint`, `crate`, `test`, `panic`, `handle`, `file system`, `raw`
and `capability`; all are invariable in the plural (`i crate`, `i lint`).
`crate` and `lint` are treated as masculine, `capability` as feminine. Native
terms are established for `modulo`, `funzione`, `chiusura`, `attributo`,
`soglia`, `ramo` and `directory di lavoro` (`directory` itself being a settled
loan).

Hazards: `panic` is kept as the English loan in the fixed phrase
`andare in panic`, because the native `panico` reads as human alarm.
`attributo` names both a Rust attribute and a Fluent attribute; catalogue
messages mean only the former. `ramo` (a counted branch) is kept apart from
`diramazioni` (branching in general) and from a version-control `branch`.
`fallback` is rendered `alternativa di ripiego` for a value and `di ripiego` as
a modifier (`diagnostica di ripiego`). `cooked string` has no Italian term; the
catalogue coins `stringa non raw`, defining it by contrast with the familiar
`stringa raw`. `complexity bump` is rendered `picco di complessità` (a peak),
which keeps the meaning but drops the road metaphor of the Bumpy Road name;
`inner doc comment` and `capability` likewise have no settled Italian
rendering. English fragments such as `{ $name }` sit inside guillemets so that
they need no article or agreement.

Worked example, from `common/locales/it/module_max_lines.ftl`:

```ftl
module_max_lines = Il modulo { $module } si estende su { $lines } righe e supera il limite consentito di { $limit }.
```

| title                | preferred                            | allowed                                  | forbidden                         |
| -------------------- | ------------------------------------ | ---------------------------------------- | --------------------------------- |
| lint                 | `lint`                               | `regola di lint`                         | —                                 |
| crate                | `crate`                              | —                                        | `cassa`; `pacchetto`              |
| module               | `modulo`                             | —                                        | —                                 |
| function             | `funzione`                           | —                                        | `metodo` (for a free function)    |
| helper function      | `funzione ausiliaria`                | `funzione di supporto`                   | —                                 |
| closure              | `chiusura`                           | `closure`                                | —                                 |
| macro expansion      | `espansione delle macro`             | `espansione di macro`                    | —                                 |
| attribute            | `attributo`                          | —                                        | `proprietà`                       |
| outer attribute      | `attributo esterno`                  | —                                        | —                                 |
| doc comment          | `commento di documentazione`         | —                                        | `commento` (unqualified)          |
| inner doc comment    | `commento di documentazione interno` | —                                        | —                                 |
| test                 | `test`                               | —                                        | `prova`                           |
| panic                | `panic`                              | `andare in panic` (verb)                 | `panico`                          |
| variant              | `variante`                           | —                                        | `variazione`                      |
| fallback             | `alternativa di ripiego`             | `di ripiego` (modifier)                  | —                                 |
| cooked string        | `stringa non raw`                    | —                                        | `stringa cotta`                   |
| escaped newline      | `ritorno a capo con escape`          | —                                        | —                                 |
| cognitive complexity | `complessità cognitiva`              | —                                        | `complessità ciclomatica`         |
| complexity bump      | `picco di complessità`               | —                                        | —                                 |
| threshold            | `soglia`                             | —                                        | `limite` (reserved for the limit) |
| branch               | `ramo`                               | `diramazioni` (for branching in general) | —                                 |
| test coverage        | `copertura dei test`                 | —                                        | —                                 |
| working directory    | `directory di lavoro`                | —                                        | `cartella di lavoro`              |
| capability           | `capability`                         | —                                        | `capacità`; `funzionalità`        |
| diagnostic           | `diagnostica`                        | `messaggio diagnostico`                  | `diagnosi`                        |

_Table 24: Italian terminology._

### Japanese (`ja`)

Japanese diagnostics use the polite `です`/`ます` register (teineigo) without
humble or honorific forms, the register the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
set for software. Conditions are stated declaratively (`…あります`,
`…必要です`); every `.help` instruction and every primary message that asks for
a change ends in the polite request form `…してください`. The reader is never
addressed with a pronoun, so no gender or number choice arises. Short span
labels (`.label`, `.suggestion`) are noun phrases or plain-form verbs without a
terminal `。`. Full-width punctuation (`、`, `。`, `「」`, `（）`) is used in
prose; a half-width space separates Japanese text from placeables, code spans,
and Latin identifiers, except next to full-width punctuation. English fragments
supplied by the lint are enclosed in `「」`. Counts take an inline counter
(`{ $count } 個`, `{ $lines } 行`), so no plural select is needed.

Most Rust vocabulary is rendered as katakana loans, in line with the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html):
`リント`, `クレート`, `モジュール`, `クロージャ`, `マクロ`, `パニック`,
`バリアント`, `フォールバック`, `テストカバレッジ`, and `ケイパビリティ`.
Established Sino-Japanese terms are kept where they exist: `関数`, `属性`,
`分岐`, `診断`, `認知的複雑度`, and `しきい値`. Identifiers such as `expect`,
`std::fs`, and `raw` stay in Latin script.

`属性` names the Rust attribute only; a Fluent attribute is a catalogue concept
that never appears in rendered text. `テスト` is a noun, so the verb sense takes
`する`. `パニック` could read as the everyday word for alarm; the catalogue
always pairs it with `する` or `を引き起こす` so the runtime sense is plain.
`分岐` covers both branch and branching, which keeps
`conditional_max_n_branches` and `macro_aware_cognitive_complexity` parallel.
Three terms are coinages with no established Japanese rendering:
`非 raw 文字列` for cooked string (named by contrast with the familiar raw
string), `複雑度の山` for complexity bump, and `内部ドキュメントコメント` for
inner doc comment. `ケイパビリティ` is preferred over `権限`, which would
suggest operating-system permissions rather than capability handles.

Worked example, from `common/locales/ja/module_max_lines.ftl`:

```ftl
module_max_lines = モジュール { $module } は { $lines } 行あり、許容される { $limit } 行を超えています。
```

| title                | preferred                  | allowed                                | forbidden                                             |
| -------------------- | -------------------------- | -------------------------------------- | ----------------------------------------------------- |
| lint                 | `リント`                   | `lint` (as an identifier or lint name) | —                                                     |
| crate                | `クレート`                 | —                                      | `パッケージ` (a Cargo package is a different concept) |
| module               | `モジュール`               | —                                      | —                                                     |
| function             | `関数`                     | —                                      | —                                                     |
| helper function      | `ヘルパー関数`             | `補助関数`                             | —                                                     |
| closure              | `クロージャ`               | `クロージャー`                         | —                                                     |
| macro expansion      | `マクロ展開`               | —                                      | —                                                     |
| attribute            | `属性`                     | `アトリビュート`                       | —                                                     |
| outer attribute      | `外部属性`                 | `アウター属性`                         | —                                                     |
| doc comment          | `ドキュメントコメント`     | `doc コメント`                         | —                                                     |
| inner doc comment    | `内部ドキュメントコメント` | `インナードキュメントコメント`         | —                                                     |
| test                 | `テスト`                   | —                                      | —                                                     |
| panic                | `パニック`                 | `パニックする` (verb)                  | —                                                     |
| variant              | `バリアント`               | `列挙子`                               | —                                                     |
| fallback             | `フォールバック`           | `代替処理`                             | —                                                     |
| cooked string        | `非 raw 文字列`            | `通常の文字列リテラル`                 | `生文字列` (means raw string)                         |
| escaped newline      | `エスケープされた改行`     | —                                      | —                                                     |
| cognitive complexity | `認知的複雑度`             | `認知複雑度`                           | `循環的複雑度` (cyclomatic complexity)                |
| complexity bump      | `複雑度の山`               | —                                      | —                                                     |
| threshold            | `しきい値`                 | `閾値`                                 | —                                                     |
| branch               | `分岐`                     | `ブランチ` (version control only)      | —                                                     |
| test coverage        | `テストカバレッジ`         | `テスト網羅率`                         | —                                                     |
| working directory    | `作業ディレクトリ`         | `カレントディレクトリ`                 | —                                                     |
| capability           | `ケイパビリティ`           | —                                      | `権限` (operating-system permission)                  |
| diagnostic           | `診断`                     | `診断メッセージ`                       | —                                                     |

_Table 25: Japanese terminology._

### Korean (`ko`)

Korean diagnostics use the formal `합니다` declarative style, and every
instruction uses the formal imperative `하십시오` (`줄이십시오`, `옮기십시오`),
the register the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for developer tools; the softer `해요` and `하세요` styles are not
used. The reader is not addressed with a pronoun. Span labels are noun phrases
or `-기` nominal forms without a full stop. Korean words are spaced normally
and Latin punctuation is used; a particle attaches directly to a placeable or
code span. Because the correct particle depends on the final sound of a value
the catalogue cannot see, sentences are arranged so that a fixed noun or
counter carries the particle (`{ $module } 모듈은`, `{ $index }번은`); where
that is not possible the dual form `이(가)` or `은(는)` is written. English
fragments supplied by the lint are enclosed in `“ ”`. Counts take an inline
counter (`{ $count }개`, `{ $lines }줄`), so no plural select is needed.

Rust vocabulary is mostly transliterated into Hangul, in line with the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html):
`린트`, `크레이트`, `모듈`, `클로저`, `매크로`, `패닉`, `배리언트`, `폴백`,
`테스트 커버리지`, and `케이퍼빌리티`. Established Sino-Korean terms are kept
for `함수`, `속성`, `분기`, `진단`, `임계값`, and `인지 복잡도`. Identifiers
stay in Latin script.

`속성` names the Rust attribute only; it is also the everyday word for
property, so the catalogue always writes `외부 속성` or names the attribute
through `{ $attribute }`. `패닉` is used with `하다` or `일으키다` to keep the
runtime sense. `분기` covers both branch and branching. `폴백` is preferred over
`대체`, which would read as plain replacement and collide with the instruction
to replace a call. Coinages with no established Korean rendering:
`비원시 문자열` for cooked string (by contrast with `원시 문자열`, raw string),
`복잡도 봉우리` for complexity bump, and `내부 문서 주석` for inner doc comment.
`케이퍼빌리티` is kept as a loan because `권한` means permission.

Worked example, from `common/locales/ko/module_max_lines.ftl`:

```ftl
module_max_lines = 모듈 { $module }의 길이는 { $lines }줄로, 허용된 { $limit }줄을 초과합니다.
```

| title                | preferred             | allowed                                | forbidden                                         |
| -------------------- | --------------------- | -------------------------------------- | ------------------------------------------------- |
| lint                 | `린트`                | `lint` (as an identifier or lint name) | —                                                 |
| crate                | `크레이트`            | —                                      | `패키지` (a Cargo package is a different concept) |
| module               | `모듈`                | —                                      | —                                                 |
| function             | `함수`                | —                                      | —                                                 |
| helper function      | `도우미 함수`         | `헬퍼 함수`                            | —                                                 |
| closure              | `클로저`              | —                                      | —                                                 |
| macro expansion      | `매크로 확장`         | —                                      | —                                                 |
| attribute            | `속성`                | `애트리뷰트`                           | —                                                 |
| outer attribute      | `외부 속성`           | —                                      | —                                                 |
| doc comment          | `문서 주석`           | `문서화 주석`                          | —                                                 |
| inner doc comment    | `내부 문서 주석`      | —                                      | —                                                 |
| test                 | `테스트`              | —                                      | —                                                 |
| panic                | `패닉`                | `패닉하다` (verb)                      | —                                                 |
| variant              | `배리언트`            | `열거값`                               | —                                                 |
| fallback             | `폴백`                | `대체 동작`                            | —                                                 |
| cooked string        | `비원시 문자열`       | `일반 문자열 리터럴`                   | `원시 문자열` (means raw string)                  |
| escaped newline      | `이스케이프된 줄바꿈` | —                                      | —                                                 |
| cognitive complexity | `인지 복잡도`         | —                                      | `순환 복잡도` (cyclomatic complexity)             |
| complexity bump      | `복잡도 봉우리`       | —                                      | —                                                 |
| threshold            | `임계값`              | `문턱값`                               | —                                                 |
| branch               | `분기`                | `브랜치` (version control only)        | —                                                 |
| test coverage        | `테스트 커버리지`     | `테스트 적용 범위`                     | —                                                 |
| working directory    | `작업 디렉터리`       | `현재 디렉터리`                        | `작업 폴더`                                       |
| capability           | `케이퍼빌리티`        | —                                      | `권한` (permission)                               |
| diagnostic           | `진단`                | `진단 메시지`                          | —                                                 |

_Table 26: Korean terminology._

### Norwegian Bokmål (`nb`)

Norwegian diagnostics are written in Bokmål, not Nynorsk, and address the
reader informally with `du`, the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
use for Norwegian software; the formal `De` never appears. Messages that
demand a change and every `.help` attribute use the bare imperative (`Flytt …`,
`Del … opp`), so the pronoun surfaces only in possessives such as
`API-ene dine`. Condition reports are declarative. Moderate Bokmål forms are
used throughout (`-en` rather than `-a` definite endings for common-gender
nouns), and quotation marks around English fragments are guillemets.

The catalogue keeps `lint`, `crate`, `closure`, `panic`, `fallback`, and
`literal` as English loans with Norwegian inflection where one is needed
(`closuren`, `fallbacker`). `modul`, `funksjon`, `hjelpefunksjon`, `attributt`,
`variant`, `terskelverdi`, `gren`, `testdekning`, and `arbeidsmappe` are
established native terms, and `håndtak` renders a handle. `kapabilitet` is used
as the adapted form of capability so that it can form compounds
(`kapabilitetsbasert`).

The main hazards are these. `gren` renders a counted branch of a predicate and
must not be replaced by `branch`, which reads as a version-control branch; the
activity is `forgrening`. `attributt` is neuter (`det ytre attributtet`), and a
Fluent attribute needs the qualifier `Fluent-attributt` in translators' notes.
`panic` stays a loan (`utløse panic`) so that it is not confused with the
everyday `panikk`. `grense` renders the configured limit and `terskelverdi` the
bump threshold. `cooked streng`, `kompleksitetstopp`,
`indre dokumentasjonskommentar`, and `escapet linjeskift` are coinages or ad
hoc borrowings with no established Norwegian rendering.

Worked example, from `common/locales/nb/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Modulen { $module } går over { $lines } linje og overskrider den tillatte grensen på { $limit }.
       *[other] Modulen { $module } går over { $lines } linjer og overskrider den tillatte grensen på { $limit }.
    }
```

| title                | preferred                       | allowed                     | forbidden                                                |
| -------------------- | ------------------------------- | --------------------------- | -------------------------------------------------------- |
| lint                 | `lint`                          | `lint-regel`                | —                                                        |
| crate                | `crate`                         | —                           | `kasse`; `pakke` (a Cargo package is a distinct concept) |
| module               | `modul`                         | —                           | —                                                        |
| function             | `funksjon`                      | —                           | —                                                        |
| helper function      | `hjelpefunksjon`                | —                           | —                                                        |
| closure              | `closure`                       | —                           | `lukking`                                                |
| macro expansion      | `makroekspansjon`               | `makroutvidelse`            | —                                                        |
| attribute            | `attributt`                     | —                           | —                                                        |
| outer attribute      | `ytre attributt`                | —                           | —                                                        |
| doc comment          | `dokumentasjonskommentar`       | `doc-kommentar`             | —                                                        |
| inner doc comment    | `indre dokumentasjonskommentar` | —                           | —                                                        |
| test                 | `test`                          | —                           | —                                                        |
| panic                | `panic`                         | `utløse panic` (verb)       | `panikk`                                                 |
| variant              | `variant`                       | —                           | —                                                        |
| fallback             | `fallback`                      | —                           | —                                                        |
| cooked string        | `cooked streng`                 | —                           | `kokt streng`                                            |
| escaped newline      | `escapet linjeskift`            | —                           | —                                                        |
| cognitive complexity | `kognitiv kompleksitet`         | —                           | —                                                        |
| complexity bump      | `kompleksitetstopp`             | —                           | —                                                        |
| threshold            | `terskelverdi`                  | —                           | `grense` (reserved for the limit)                        |
| branch               | `gren`                          | `forgrening` (the activity) | `branch`                                                 |
| test coverage        | `testdekning`                   | —                           | —                                                        |
| working directory    | `arbeidsmappe`                  | `arbeidskatalog`            | —                                                        |
| capability           | `kapabilitet`                   | —                           | `evne`                                                   |
| diagnostic           | `diagnosemelding`               | `diagnose`                  | —                                                        |

_Table 27: Norwegian Bokmål terminology._

### Dutch (`nl`)

Dutch diagnostics address the reader informally with `je`, the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
adopt for current Dutch software, and never switch to `u`. Messages that
demand a change and every `.help` attribute use the bare imperative
(`Verplaats …`, `Splits … op`), which carries no pronoun at all; `je` surfaces
only in possessives such as `je API's`. Condition reports are declarative and
have no addressee, so no sentence genders the reader. Quotation marks around
English fragments are the double typographic pair.

Dutch technical writing borrows freely from English, and the catalogue keeps
`lint`, `crate`, `closure`, `panic`, `fallback`, `literal`, `capability`, and
`cooked string` as loans. `module`, `functie`, `hulpfunctie`, `attribuut`,
`variant`, `drempelwaarde`, `tak`, `testdekking`, and `werkmap` are established
native terms. Compounds built on a frozen identifier take a hyphen
(`std::fs`-`bewerking`, `concat!()`-`argumenten`).

The main hazards are these. `tak` renders a counted branch of a predicate and
must not be replaced by `branch`, which Dutch developers read as a
version-control branch; the abstract activity is `vertakking`. `attribuut`
serves for a Rust attribute, and a Fluent attribute needs the qualifier
`Fluent-attribuut` in translators' notes. `documentatiecommentaar` is a neuter
mass noun in the catalogue, so articles and relative pronouns follow `het` and
`dat`. `limiet` renders the configured limit and `drempelwaarde` the bump
threshold; the two must stay distinct. `cooked string`, `complexiteitspiek`,
`binnenste documentatiecommentaar`, and `ge-escapete newline` are coinages or
ad hoc borrowings with no established Dutch rendering.

Worked example, from `common/locales/nl/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Module { $module } beslaat { $lines } regel en overschrijdt daarmee de toegestane limiet van { $limit }.
       *[other] Module { $module } beslaat { $lines } regels en overschrijdt daarmee de toegestane limiet van { $limit }.
    }
```

| title                | preferred                          | allowed                        | forbidden                                                |
| -------------------- | ---------------------------------- | ------------------------------ | -------------------------------------------------------- |
| lint                 | `lint`                             | `lintregel`                    | —                                                        |
| crate                | `crate`                            | —                              | `krat`; `pakket` (a Cargo package is a distinct concept) |
| module               | `module`                           | —                              | —                                                        |
| function             | `functie`                          | —                              | —                                                        |
| helper function      | `hulpfunctie`                      | —                              | —                                                        |
| closure              | `closure`                          | —                              | `afsluiting`                                             |
| macro expansion      | `macro-expansie`                   | —                              | —                                                        |
| attribute            | `attribuut`                        | —                              | —                                                        |
| outer attribute      | `buitenste attribuut`              | —                              | —                                                        |
| doc comment          | `documentatiecommentaar`           | `doc-commentaar`               | —                                                        |
| inner doc comment    | `binnenste documentatiecommentaar` | —                              | —                                                        |
| test                 | `test`                             | —                              | —                                                        |
| panic                | `panic`                            | `een panic veroorzaken` (verb) | `paniek`                                                 |
| variant              | `variant`                          | —                              | —                                                        |
| fallback             | `fallback`                         | `terugvaloptie`                | —                                                        |
| cooked string        | `cooked string`                    | —                              | `gekookte string`                                        |
| escaped newline      | `ge-escapete newline`              | `ge-escapet regeleinde`        | —                                                        |
| cognitive complexity | `cognitieve complexiteit`          | —                              | —                                                        |
| complexity bump      | `complexiteitspiek`                | —                              | —                                                        |
| threshold            | `drempelwaarde`                    | —                              | `limiet` (reserved for the limit)                        |
| branch               | `tak`                              | `vertakking` (the activity)    | `branch`                                                 |
| test coverage        | `testdekking`                      | —                              | —                                                        |
| working directory    | `werkmap`                          | `werkdirectory`                | —                                                        |
| capability           | `capability`                       | —                              | `bekwaamheid`                                            |
| diagnostic           | `diagnose`                         | `diagnostisch bericht`         | —                                                        |

_Table 28: Dutch terminology._

### Polish (`pl`)

Polish diagnostics use the informal-neutral second person singular imperative
without a pronoun (`Wyodrębnij`, `Podziel`, `Przenieś`), the instruction form
the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for Polish software; `Pan`/`Pani` forms and capitalized pronouns are
not used. Conditions are stated with impersonal `-no`/`-to` forms (`Wykryto`)
or plain present-tense statements, which also avoids gendering the reader.
Quotation marks are `„…”`.

`lint` is kept as a loan and declined as a masculine noun (`lintu`, `linty`).
`crate` is rendered `skrzynka`, the term used by the Polish entry among the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html),
with the English loan allowed. Other vocabulary is native: `moduł`, `funkcja`,
`domknięcie`, `makro`, `atrybut`, `komentarz dokumentacyjny`, `wariant`,
`gałąź`, and `katalog roboczy`.

Polish plural agreement distinguishes 1, 2–4 (except 12–14), 5 and above, and
fractions, so counted sentences are four-way selects. The limit sentence is
built with `do` plus the genitive, where `gałęzi` happens to be identical in
every category; the select is kept so that the structure matches the other
counted messages. English fragments sit in quotation marks after a declinable
head noun (`wyrażenie „{ $name }”`, `elementów „{ $subject }”`). `cooked string`
(`przetwarzany ciąg znaków`), `complexity bump` (`wybój złożoności`, from
`wyboista droga`), and `capability` (`zdolność`) are coinages; `uprawnienie` is
avoided for `capability` because it means a permission. `escaped newline` is
spelt out descriptively because the borrowed participle `escapowany` is
colloquial. `panika` is the noun used with `wywołać`; `test` is always the noun;
`atrybut` always means a Rust attribute.

Worked example, from `common/locales/pl/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Moduł { $module } ma { $lines } wiersz, co przekracza dozwolony limit { $limit }.
        [few] Moduł { $module } ma { $lines } wiersze, co przekracza dozwolony limit { $limit }.
        [many] Moduł { $module } ma { $lines } wierszy, co przekracza dozwolony limit { $limit }.
       *[other] Moduł { $module } ma { $lines } wiersza, co przekracza dozwolony limit { $limit }.
    }
```

| title                | preferred                                              | allowed                                                | forbidden                                   |
| -------------------- | ------------------------------------------------------ | ------------------------------------------------------ | ------------------------------------------- |
| lint                 | `lint`                                                 | —                                                      | —                                           |
| crate                | `skrzynka`                                             | `crate`                                                | `paczka` (means a Cargo package)            |
| module               | `moduł`                                                | —                                                      | —                                           |
| function             | `funkcja`                                              | —                                                      | —                                           |
| helper function      | `funkcja pomocnicza`                                   | —                                                      | —                                           |
| closure              | `domknięcie`                                           | —                                                      | `zamknięcie`                                |
| macro expansion      | `rozwinięcie makra`                                    | `ekspansja makra`                                      | —                                           |
| attribute            | `atrybut`                                              | —                                                      | —                                           |
| outer attribute      | `atrybut zewnętrzny`                                   | —                                                      | —                                           |
| doc comment          | `komentarz dokumentacyjny`                             | —                                                      | —                                           |
| inner doc comment    | `wewnętrzny komentarz dokumentacyjny`                  | —                                                      | —                                           |
| test                 | `test`                                                 | —                                                      | —                                           |
| panic                | `panika`                                               | `panic` (in code references)                           | —                                           |
| variant              | `wariant`                                              | —                                                      | —                                           |
| fallback             | `rozwiązanie zastępcze`                                | `zastępczy` (adjective, as in `diagnostyka zastępcza`) | —                                           |
| cooked string        | `przetwarzany ciąg znaków`                             | —                                                      | `surowy ciąg znaków`                        |
| escaped newline      | `znak nowego wiersza poprzedzony ukośnikiem odwrotnym` | `escapowany znak nowego wiersza`                       | —                                           |
| cognitive complexity | `złożoność poznawcza`                                  | `złożoność kognitywna`                                 | —                                           |
| complexity bump      | `wybój złożoności`                                     | —                                                      | —                                           |
| threshold            | `próg`                                                 | —                                                      | `limit` (reserved for the configured limit) |
| branch               | `gałąź`                                                | —                                                      | —                                           |
| test coverage        | `pokrycie testami`                                     | —                                                      | —                                           |
| working directory    | `katalog roboczy`                                      | —                                                      | `folder roboczy`                            |
| capability           | `zdolność`                                             | —                                                      | `uprawnienie`                               |
| diagnostic           | `diagnostyka`                                          | `komunikat diagnostyczny`                              | —                                           |

_Table 29: Polish terminology._

### Portuguese, Brazil (`pt-BR`)

The `pt-BR` catalogue follows Brazilian usage and differs from `pt-PT` in
address, vocabulary, orthography and punctuation. It addresses the reader as
`você` without writing the pronoun: `.help` text uses the third-person
imperative (`Extraia`, `Divida`, `Mova`, `Adicione`), the instruction form
Brazilian software conventionally uses, and possessives are `seu`/`suas`
(`suas APIs`). `tu` forms never appear. Primary messages and notes state the
condition impersonally, and the gerund is used freely (`excedendo`,
`adicionando`). English-style double quotation marks (`“{ $name }”`) enclose
quoted fragments, where `pt-PT` uses angle quotation marks. Code keeps ASCII
backticks. No sentence requires gender agreement with the reader.

Brazilian developer prose keeps many English loans, and the catalogue does the
same for `lint`, `crate`, `closure`, `string`, `fallback`, `handle` and
`escape`, in line with the community Brazilian Portuguese translation of _The
Rust Programming Language_ listed in the
[book's translation appendix](https://doc.rust-lang.org/book/appendix-06-translation.html).
`crate`, `lint`, `fallback` and `handle` are masculine; `closure` and `string`
are feminine. Brazilian vocabulary and spelling are used where the variants
differ: `sistema de arquivos` (not `sistema de ficheiros`), `usuários` (not
`utilizadores`), `seções` (not `secções`), `detectada` (not `detetada`),
`por padrão` (not `por predefinição`), `menores` (not `mais pequenos`) and
`revisar` (not `rever`).

Hazards: `pânico` carries the everyday sense of alarm; the catalogue pairs it
with a code identifier or the fixed phrase `entrar em pânico`. `atributo` names
both a Rust attribute and a Fluent attribute; catalogue messages mean only the
former. `ramificação` renders a counted branch and, in the plural, branching in
general; it must not be replaced by `branch`, which in Brazilian usage means a
version-control branch. `limiar` (threshold) stays distinct from `limite`
(limit). The crate name `camino` resembles `caminho` (path), so the catalogue
writes `caminhos do` followed by the name in backticks. `cooked string`
(`string interpretada`), `complexity bump` (`lombada de complexidade`, from the
Brazilian word for a speed bump), `inner doc comment` and `capability`
(`capacidade`) are coinages or calques without settled usage.

Worked example, from `common/locales/pt-BR/module_max_lines.ftl`:

```ftl
module_max_lines = O módulo { $module } abrange { $lines } linhas, excedendo o limite permitido de { $limit }.
```

| title                | preferred                            | allowed                   | forbidden                                    |
| -------------------- | ------------------------------------ | ------------------------- | -------------------------------------------- |
| lint                 | `lint`                               | `regra de lint`           | —                                            |
| crate                | `crate`                              | —                         | `caixote`; `pacote`                          |
| module               | `módulo`                             | —                         | —                                            |
| function             | `função`                             | —                         | `método` (for a free function)               |
| helper function      | `função auxiliar`                    | —                         | —                                            |
| closure              | `closure`                            | —                         | `fechamento`                                 |
| macro expansion      | `expansão de macros`                 | —                         | —                                            |
| attribute            | `atributo`                           | —                         | `propriedade`                                |
| outer attribute      | `atributo externo`                   | —                         | —                                            |
| doc comment          | `comentário de documentação`         | —                         | `comentário` (unqualified)                   |
| inner doc comment    | `comentário de documentação interno` | —                         | —                                            |
| test                 | `teste`                              | —                         | `ensaio`                                     |
| panic                | `pânico`                             | `entrar em pânico` (verb) | —                                            |
| variant              | `variante`                           | —                         | `variação`                                   |
| fallback             | `fallback`                           | `de fallback` (modifier)  | `backup`                                     |
| cooked string        | `string interpretada`                | —                         | `string cozida`                              |
| escaped newline      | `quebra de linha com escape`         | —                         | —                                            |
| cognitive complexity | `complexidade cognitiva`             | —                         | `complexidade ciclomática`                   |
| complexity bump      | `lombada de complexidade`            | —                         | —                                            |
| threshold            | `limiar`                             | —                         | `limite` (reserved for the limit)            |
| branch               | `ramificação`                        | —                         | `branch` (reads as a version-control branch) |
| test coverage        | `cobertura de testes`                | —                         | —                                            |
| working directory    | `diretório de trabalho`              | —                         | `pasta de trabalho`                          |
| capability           | `capacidade`                         | —                         | `funcionalidade`                             |
| diagnostic           | `diagnóstico`                        | —                         | —                                            |

_Table 30: Portuguese (Brazil) terminology._

### Portuguese, Portugal (`pt-PT`)

The `pt-PT` catalogue follows European Portuguese usage and differs from
`pt-BR` in address, vocabulary, orthography and punctuation. It does not
address the reader directly: `.help` text uses the impersonal infinitive
(`Extrair`, `Dividir`, `Mover`, `Adicionar`), the instruction form European
Portuguese software and documentation conventionally prefer, and neither `tu`
nor `você` appears. Primary messages and notes state the condition
impersonally, and constructions with `a` plus infinitive or a relative clause
replace the Brazilian gerund where it would sound foreign
(`para que sejam os chamadores a escolher`). Angle quotation marks
(`«{ $name }»`) enclose quoted fragments, where `pt-BR` uses double quotation
marks. Spelling follows the 1990 orthographic agreement as applied in Portugal
(`detetada`, `atualmente`, `diretório`). Code keeps ASCII backticks. No
sentence requires gender agreement with the reader.

`lint`, `crate`, `closure`, `string`, `handle` and `escape` stay as English
loans, with the same genders as in `pt-BR`. European vocabulary and spelling
are used where the variants differ: `sistema de ficheiros` (not
`sistema de arquivos`), `utilizadores` (not `usuários`), `secções` (not
`seções`), `detetada` (not `detectada`), `por predefinição` (not `por padrão`),
`mais pequenos` (not `menores`), `rever` (not `revisar`), `partilhados` (not
`compartilhados`) and `mudança de linha` (not `quebra de linha`). `fallback` is
rendered natively as `alternativa de contingência` instead of being borrowed.

Hazards: `pânico` carries the everyday sense of alarm; the catalogue pairs it
with a code identifier or the fixed phrase `entrar em pânico`. `atributo` names
both a Rust attribute and a Fluent attribute; catalogue messages mean only the
former. `ramo` (a counted branch) stays distinct from `ramificação` (branching
in general) and from a version-control `branch`. `limiar` (threshold) stays
distinct from `limite` (limit). The crate name `camino` resembles `caminho`
(path), so the catalogue writes `caminhos do` followed by the name in backticks.
`cooked string` (`string interpretada`), `complexity bump`
(`lomba de complexidade`, from the European word for a speed bump, where
`pt-BR` uses `lombada`), `inner doc comment` and `capability` (`capacidade`)
are coinages or calques without settled usage.

Worked example, from `common/locales/pt-PT/module_max_lines.ftl`:

```ftl
module_max_lines = O módulo { $module } abrange { $lines } linhas, excedendo o limite permitido de { $limit }.
```

| title                | preferred                            | allowed                                  | forbidden                         |
| -------------------- | ------------------------------------ | ---------------------------------------- | --------------------------------- |
| lint                 | `lint`                               | `regra de lint`                          | —                                 |
| crate                | `crate`                              | —                                        | `caixote`; `pacote`               |
| module               | `módulo`                             | —                                        | —                                 |
| function             | `função`                             | —                                        | `método` (for a free function)    |
| helper function      | `função auxiliar`                    | —                                        | —                                 |
| closure              | `closure`                            | `fecho`                                  | `encerramento`                    |
| macro expansion      | `expansão de macros`                 | —                                        | —                                 |
| attribute            | `atributo`                           | —                                        | `propriedade`                     |
| outer attribute      | `atributo externo`                   | —                                        | —                                 |
| doc comment          | `comentário de documentação`         | —                                        | `comentário` (unqualified)        |
| inner doc comment    | `comentário de documentação interno` | —                                        | —                                 |
| test                 | `teste`                              | —                                        | `ensaio`                          |
| panic                | `pânico`                             | `entrar em pânico` (verb)                | —                                 |
| variant              | `variante`                           | —                                        | `variação`                        |
| fallback             | `alternativa de contingência`        | `de contingência` (modifier)             | `cópia de segurança`              |
| cooked string        | `string interpretada`                | `cadeia de carateres interpretada`       | `string cozida`                   |
| escaped newline      | `mudança de linha com escape`        | —                                        | —                                 |
| cognitive complexity | `complexidade cognitiva`             | —                                        | `complexidade ciclomática`        |
| complexity bump      | `lomba de complexidade`              | —                                        | —                                 |
| threshold            | `limiar`                             | —                                        | `limite` (reserved for the limit) |
| branch               | `ramo`                               | `ramificação` (for branching in general) | —                                 |
| test coverage        | `cobertura de testes`                | —                                        | —                                 |
| working directory    | `diretório de trabalho`              | —                                        | `pasta de trabalho`               |
| capability           | `capacidade`                         | —                                        | `funcionalidade`                  |
| diagnostic           | `diagnóstico`                        | —                                        | —                                 |

_Table 31: Portuguese (Portugal) terminology._

### Romanian (`ro`)

Romanian diagnostics address the reader with the polite plural, the register of
`dumneavoastră`, which follows the convention of the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
for professional software. `.help` text therefore uses the second-person
plural imperative (`Extrageți`, `Împărțiți`, `Mutați`, `Adăugați`), and the one
possessive that points at the reader is `dumneavoastră`; the familiar `tu`
never appears. Primary messages and notes state the condition impersonally,
often in the passive (`A fost detectată`). Section comments use the verbal noun
(`Restricționarea…`). Romanian quotation marks (`„{ $name }”`) enclose quoted
fragments, and code keeps ASCII backticks. The catalogue uses the comma-below
letters `ș` and `ț`, never the cedilla forms, and the `â`/`sunt` orthography.
No sentence requires gender agreement with the reader.

Plural agreement needs three forms. The engine's `one` category takes the
singular; `few` (0, 2–19 and numbers ending in 01–19) takes the bare plural
(`3 linii`); and `other` (20 and above otherwise) requires the preposition `de`
between numeral and noun (`20 de linii`). For that reason `module_max_lines`
carries a select on `{ $lines }` although the English source has none.

`lint`, `crate` and `handle` stay as English loans and take hyphenated Romanian
endings where the spelling would otherwise be unclear (`crate-uri`,
`handle-uri`, `API-uri`), while `lint` inflects directly (`linturi`, `lintul`).
Native terms are established for `modul`, `funcție`, `atribut`, `prag`,
`ramură` and `director de lucru`; `macrouri` is the plural of the settled loan
`macro`.

Hazards: `panică` carries the everyday sense of alarm; the catalogue pairs it
with a code identifier or the fixed phrase `a intra în panică`. `atribut` names
both a Rust attribute and a Fluent attribute; catalogue messages mean only the
former. `ramură` (a counted branch) stays distinct from `ramificare` (branching
in general). `director` also means a manager; `director de lucru` is always
written in full. `închidere` renders `closure` but also means closing or
shutdown, so the loan `closure` is an allowed alternative. `cooked string`
(`șir interpretat`), `escaped newline` (`linie nouă escapată`),
`complexity bump` (`denivelare de complexitate`), `inner doc comment` and
`capability` (`capabilitate`) are coinages or calques without settled Romanian
usage.

Worked example, from `common/locales/ro/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Modulul { $module } se întinde pe { $lines } linie, depășind limita permisă de { $limit }.
        [few] Modulul { $module } se întinde pe { $lines } linii, depășind limita permisă de { $limit }.
       *[other] Modulul { $module } se întinde pe { $lines } de linii, depășind limita permisă de { $limit }.
    }
```

| title                | preferred                             | allowed                                 | forbidden                         |
| -------------------- | ------------------------------------- | --------------------------------------- | --------------------------------- |
| lint                 | `lint`                                | `regulă de lint`                        | —                                 |
| crate                | `crate`                               | —                                       | `ladă`; `pachet`                  |
| module               | `modul`                               | —                                       | —                                 |
| function             | `funcție`                             | —                                       | `metodă` (for a free function)    |
| helper function      | `funcție auxiliară`                   | `funcție ajutătoare`                    | —                                 |
| closure              | `închidere`                           | `closure`                               | —                                 |
| macro expansion      | `expandarea macrourilor`              | `expandare de macrouri`                 | —                                 |
| attribute            | `atribut`                             | —                                       | `proprietate`                     |
| outer attribute      | `atribut exterior`                    | —                                       | —                                 |
| doc comment          | `comentariu de documentație`          | —                                       | `comentariu` (unqualified)        |
| inner doc comment    | `comentariu de documentație interior` | —                                       | —                                 |
| test                 | `test`                                | —                                       | `probă`                           |
| panic                | `panică`                              | `a intra în panică` (verb)              | —                                 |
| variant              | `variantă`                            | —                                       | `variație`                        |
| fallback             | `soluție de rezervă`                  | `de rezervă` (modifier)                 | `copie de rezervă`                |
| cooked string        | `șir interpretat`                     | —                                       | `șir gătit`                       |
| escaped newline      | `linie nouă escapată`                 | —                                       | —                                 |
| cognitive complexity | `complexitate cognitivă`              | —                                       | `complexitate ciclomatică`        |
| complexity bump      | `denivelare de complexitate`          | —                                       | —                                 |
| threshold            | `prag`                                | —                                       | `limită` (reserved for the limit) |
| branch               | `ramură`                              | `ramificare` (for branching in general) | —                                 |
| test coverage        | `acoperirea testelor`                 | —                                       | —                                 |
| working directory    | `director de lucru`                   | —                                       | `dosar de lucru`                  |
| capability           | `capabilitate`                        | —                                       | `capacitate`; `funcționalitate`   |
| diagnostic           | `diagnostic`                          | —                                       | `diagnoză`                        |

_Table 32: Romanian terminology._

### Russian (`ru`)

Russian diagnostics address the reader with the polite plural `вы`, written in
lower case, and `.help` uses the plural imperative (`Вынесите`, `Разделите`,
`Переместите`), following the convention the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for Russian software. Conditions are stated with short passive
participles or impersonal forms (`Обнаружено`, `не учитывается`), which keeps
the text gender-neutral. Quotation marks are `«…»`, and the letter `ё` is
written where it occurs.

Rust vocabulary largely follows the usage of the Russian entry among the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html):
`крейт` and `линт` are transliterated loans that decline as ordinary masculine
nouns, while `модуль`, `функция`, `замыкание`, `макрос`, `атрибут`, `вариант`,
and `паника` are established terms. Identifiers and paths always stay in Latin
script.

Russian numerals govern case (1, 2–4, 5 and above, fractions), so each counted
sentence is a four-way select; the limit sentence uses `до` plus the genitive
and the note uses the accusative. The principal hazard is that `строка` means
both a line and a string: the catalogue reserves `строка` for a line and always
says `строковый литерал` for a string, so `cooked string` becomes the coinage
`обрабатываемый строковый литерал`. `fallback` is rendered
`резервное поведение` and never `запасной вариант`, because `вариант` is
reserved for an enum variant. `complexity bump` (`ухаб сложности`) and
`capability` (`возможность`) are coinages; `разрешение` is avoided for
`capability` because it means a permission. English fragments sit in `«…»`
after a declinable head noun (`выражение «{ $name }»`,
`элементов «{ $subject }»`). `атрибут` always means a Rust attribute.

Worked example, from `common/locales/ru/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Модуль { $module } занимает { $lines } строку, что превышает допустимый лимит { $limit }.
        [few] Модуль { $module } занимает { $lines } строки, что превышает допустимый лимит { $limit }.
        [many] Модуль { $module } занимает { $lines } строк, что превышает допустимый лимит { $limit }.
       *[other] Модуль { $module } занимает { $lines } строки, что превышает допустимый лимит { $limit }.
    }
```

| title                | preferred                                | allowed                                                | forbidden                                   |
| -------------------- | ---------------------------------------- | ------------------------------------------------------ | ------------------------------------------- |
| lint                 | `линт`                                   | `lint`                                                 | —                                           |
| crate                | `крейт`                                  | —                                                      | `пакет` (means a Cargo package)             |
| module               | `модуль`                                 | —                                                      | —                                           |
| function             | `функция`                                | —                                                      | —                                           |
| helper function      | `вспомогательная функция`                | —                                                      | —                                           |
| closure              | `замыкание`                              | —                                                      | —                                           |
| macro expansion      | `раскрытие макроса`                      | `развёртывание макроса`                                | —                                           |
| attribute            | `атрибут`                                | —                                                      | —                                           |
| outer attribute      | `внешний атрибут`                        | —                                                      | —                                           |
| doc comment          | `документирующий комментарий`            | `док-комментарий` (informal)                           | —                                           |
| inner doc comment    | `внутренний документирующий комментарий` | —                                                      | —                                           |
| test                 | `тест`                                   | —                                                      | —                                           |
| panic                | `паника`                                 | —                                                      | —                                           |
| variant              | `вариант`                                | —                                                      | —                                           |
| fallback             | `резервное поведение`                    | `резервный` (adjective, as in `резервная диагностика`) | `запасной вариант`                          |
| cooked string        | `обрабатываемый строковый литерал`       | —                                                      | `обрабатываемая строка`; `сырая строка`     |
| escaped newline      | `экранированный перевод строки`          | —                                                      | —                                           |
| cognitive complexity | `когнитивная сложность`                  | —                                                      | —                                           |
| complexity bump      | `ухаб сложности`                         | —                                                      | —                                           |
| threshold            | `порог`                                  | —                                                      | `лимит` (reserved for the configured limit) |
| branch               | `ветвь`                                  | `ветка` (informal)                                     | —                                           |
| test coverage        | `покрытие тестами`                       | `тестовое покрытие`                                    | —                                           |
| working directory    | `рабочий каталог`                        | `рабочая директория`                                   | `папка`                                     |
| capability           | `возможность`                            | —                                                      | `разрешение`                                |
| diagnostic           | `диагностика`                            | `диагностическое сообщение`                            | —                                           |

_Table 33: Russian terminology._

### Swedish (`sv`)

Swedish diagnostics address the reader informally with `du`, the universal form
in Swedish software and the one the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
use; `ni` never appears. Messages that demand a change and every `.help`
attribute use the bare imperative (`Flytta …`, `Dela upp …`), so the pronoun
surfaces only in possessives such as `dina API:er`. Condition reports are
declarative. Swedish typography is followed: both quotation marks are the
right-leaning double mark, and an abbreviation takes a colon before its ending
(`API:er`).

The catalogue keeps `lint`, `crate`, `closure`, `panic`, `fallback`, and
`literal` as English loans with Swedish inflection where one is needed
(`linten`, `lintar`, `fallbackar`). `modul`, `funktion`, `hjälpfunktion`,
`attribut`, `variant`, `tröskelvärde`, `gren`, `testtäckning`, and
`arbetskatalog` are established native terms, and `handtag` renders a handle.
`kapabilitet` is used as the adapted form of capability so that it can form
compounds (`kapabilitetsbaserad`).

The main hazards are these. `gren` renders a counted branch of a predicate and
must not be replaced by `branch`, which reads as a version-control branch; the
activity is `förgrening`. `attribut` is neuter (`det yttre attributet`), and a
Fluent attribute needs the qualifier `Fluent-attribut` in translators' notes.
`panic` stays a loan (`orsaka panic`) so that it is not confused with the
everyday `panik`. `test` is neuter in the catalogue (`ett test`), and the
choice must not alternate with the common-gender form. `gräns` renders the
configured limit and `tröskelvärde` the bump threshold. `cooked sträng`,
`komplexitetstopp`, `inre dokumentationskommentar`, and `escapad radbrytning`
are coinages or ad hoc borrowings with no established Swedish rendering.

Worked example, from `common/locales/sv/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Modulen { $module } omfattar { $lines } rad och överskrider den tillåtna gränsen på { $limit }.
       *[other] Modulen { $module } omfattar { $lines } rader och överskrider den tillåtna gränsen på { $limit }.
    }
```

| title                | preferred                      | allowed                     | forbidden                                               |
| -------------------- | ------------------------------ | --------------------------- | ------------------------------------------------------- |
| lint                 | `lint`                         | `lintregel`                 | —                                                       |
| crate                | `crate`                        | —                           | `låda`; `paket` (a Cargo package is a distinct concept) |
| module               | `modul`                        | —                           | —                                                       |
| function             | `funktion`                     | —                           | —                                                       |
| helper function      | `hjälpfunktion`                | —                           | —                                                       |
| closure              | `closure`                      | —                           | `stängning`                                             |
| macro expansion      | `makroexpansion`               | —                           | —                                                       |
| attribute            | `attribut`                     | —                           | —                                                       |
| outer attribute      | `yttre attribut`               | —                           | —                                                       |
| doc comment          | `dokumentationskommentar`      | `doc-kommentar`             | —                                                       |
| inner doc comment    | `inre dokumentationskommentar` | —                           | —                                                       |
| test                 | `test`                         | —                           | —                                                       |
| panic                | `panic`                        | `orsaka panic` (verb)       | `panik`                                                 |
| variant              | `variant`                      | —                           | —                                                       |
| fallback             | `fallback`                     | `reservlösning`             | —                                                       |
| cooked string        | `cooked sträng`                | —                           | `kokt sträng`                                           |
| escaped newline      | `escapad radbrytning`          | —                           | —                                                       |
| cognitive complexity | `kognitiv komplexitet`         | —                           | —                                                       |
| complexity bump      | `komplexitetstopp`             | —                           | —                                                       |
| threshold            | `tröskelvärde`                 | —                           | `gräns` (reserved for the limit)                        |
| branch               | `gren`                         | `förgrening` (the activity) | `branch`                                                |
| test coverage        | `testtäckning`                 | —                           | —                                                       |
| working directory    | `arbetskatalog`                | `arbetsmapp`                | —                                                       |
| capability           | `kapabilitet`                  | —                           | `förmåga`                                               |
| diagnostic           | `diagnostik`                   | `diagnostikmeddelande`      | —                                                       |

_Table 34: Swedish terminology._

### Thai (`th`)

<!-- markdownlint-disable MD013 MD060 -->

Thai diagnostics use a neutral written register without the polite particles
`ครับ` and `ค่ะ` and without personal pronouns, so no gendered form is needed; the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe the same convention for software strings. Conditions are plain
statements, and instructions open with the bare verb (`แยก`, `ย้าย`, `เพิ่ม`)
rather than `กรุณา` or `โปรด`, which would read as ceremonial when repeated
across diagnostics. Thai sentences carry no terminal full stop, so messages,
notes, and help text end without one; a space separates clauses. Thai words are
not separated by spaces, but a space is placed on each side of a placeable,
code span, Latin identifier, or number. English fragments supplied by the lint
are enclosed in `“ ”`. Counts take a classifier (`{ $branches } สาขา`,
`{ $lines } บรรทัด`, `{ $count } แห่ง`), so no plural select is needed. Digits
are Western Arabic.

Thai developer writing keeps many Rust terms in Latin script, and the catalogue
follows that practice for `lint`, `crate`, `panic`, `raw`, and `escape`, as
well as all identifiers. Transliterated loans with settled spellings are used
for `โมดูล`, `ฟังก์ชัน`, `แอตทริบิวต์`, `แมโคร`, `คอมเมนต์`, `โคลเชอร์`, `แวเรียนต์`,
`ไดเรกทอรี`, and `ลิเทอรัล`; native words are used for `การทดสอบ`, `สาขา`, `เกณฑ์`,
and `ข้อความวินิจฉัย`.

`สาขา` renders branch, and the noun `การแตกสาขา` renders branching; `กิ่ง` is
avoided because it suggests a tree limb. `การทดสอบ` is the noun for a test,
while the bare `ทดสอบ` is the verb. `panic` stays in Latin script and is used
directly as a verb, as Thai developer prose does. `ขีดความสามารถ` renders
capability and is kept distinct from `สิทธิ์`, which means permission. `ทางสำรอง`
renders fallback. Coinages with no established Thai rendering: `สตริงที่ไม่ใช่ raw`
for cooked string, `เนินความซับซ้อน` for complexity bump, `คอมเมนต์เอกสารภายใน` for
inner doc comment, and `ความซับซ้อนเชิงการรู้คิด` for cognitive complexity.

Worked example, from `common/locales/th/module_max_lines.ftl`:

```ftl
module_max_lines = โมดูล { $module } ยาว { $lines } บรรทัด เกินขีดจำกัดที่อนุญาตคือ { $limit } บรรทัด
```

| title                | preferred                   | allowed                | forbidden                                         |
| -------------------- | --------------------------- | ---------------------- | ------------------------------------------------- |
| lint                 | `lint`                      | `ลินต์`                  | —                                                 |
| crate                | `crate`                     | `เครต`                 | `แพ็กเกจ` (a Cargo package is a different concept) |
| module               | `โมดูล`                      | —                      | —                                                 |
| function             | `ฟังก์ชัน`                     | —                      | —                                                 |
| helper function      | `ฟังก์ชันตัวช่วย`                | `ฟังก์ชันช่วยเหลือ`         | —                                                 |
| closure              | `โคลเชอร์`                   | `closure`              | —                                                 |
| macro expansion      | `การขยายแมโคร`              | —                      | —                                                 |
| attribute            | `แอตทริบิวต์`                  | —                      | —                                                 |
| outer attribute      | `แอตทริบิวต์ภายนอก`            | —                      | —                                                 |
| doc comment          | `คอมเมนต์เอกสาร`             | `doc comment`          | —                                                 |
| inner doc comment    | `คอมเมนต์เอกสารภายใน`        | —                      | —                                                 |
| test                 | `การทดสอบ`                  | `เทสต์`                 | —                                                 |
| panic                | `panic`                     | `แพนิก`                 | —                                                 |
| variant              | `แวเรียนต์`                   | `variant`              | `ตัวแปร` (means variable)                          |
| fallback             | `ทางสำรอง`                  | `fallback`             | —                                                 |
| cooked string        | `สตริงที่ไม่ใช่ raw`             | `สตริงปกติ`              | `สตริงดิบ` (means raw string)                       |
| escaped newline      | `อักขระขึ้นบรรทัดใหม่แบบ escape` | —                      | —                                                 |
| cognitive complexity | `ความซับซ้อนเชิงการรู้คิด`        | `cognitive complexity` | —                                                 |
| complexity bump      | `เนินความซับซ้อน`              | —                      | —                                                 |
| threshold            | `เกณฑ์`                      | `ค่าขีดแบ่ง`              | —                                                 |
| branch               | `สาขา`                      | —                      | `กิ่ง`                                              |
| test coverage        | `ความครอบคลุมของการทดสอบ`    | `test coverage`        | —                                                 |
| working directory    | `ไดเรกทอรีทำงาน`             | `ไดเรกทอรีปัจจุบัน`        | —                                                 |
| capability           | `ขีดความสามารถ`              | `capability`           | `สิทธิ์` (permission)                                |
| diagnostic           | `ข้อความวินิจฉัย`               | `การวินิจฉัย`             | —                                                 |

_Table 35: Thai terminology._

<!-- markdownlint-enable MD013 MD060 -->

### Turkish (`tr`)

Turkish diagnostics address the reader with the polite second-person plural
(`siz`), which is the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
establish for software. `.help` text therefore uses the polite imperative in
`-in`/`-ın` (`ayıklayın`, `taşıyın`, `bölün`), never the familiar `sen` form.
Primary messages and notes state the condition impersonally in the present
continuous or with the necessity suffix (`aşıyor`, `başlamalıdır`). Turkish has
no grammatical gender, so no inclusivity device is needed. Nouns stay singular
after a numeral, so the `one` and `other` variants of each plural select carry
identical wording; both are still listed because the plural engine defines both.

Established native terms cover most of the vocabulary: `modül`, `işlev`,
`öznitelik`, `dal`, `eşik`, `çalışma dizini`. `lint` and `crate` stay as
English loans in Latin script and take Turkish suffixes after an apostrophe
(`crate’ler`). `test`, `makro`, `varyant`, and `panik` are adapted loans.

The main hazard is casing. Turkish distinguishes dotted `i`/`İ` from dotless
`ı`/`I`, so an identifier must never be re-cased or sentence-capitalized:
`Utf8PathBuf`, `None`, and every interpolated name keep their exact bytes. Case
suffixes are not attached to placeables, because vowel harmony cannot be
predicted for an interpolated value; the catalogue follows the value with a
head noun that carries the suffix instead (`{ $module } modülünü`). `öznitelik`
is used only for a Rust attribute; a Fluent attribute is not named in the
catalogue. `yedek` renders `fallback` and must not be read as a backup copy.
`kapanış` for `closure`, `işlenmiş dize` for `cooked string`,
`karmaşıklık tümseği` for `complexity bump`, `iç belge yorumu` for
`inner doc comment`, and `yetenek` for `capability` are coinages with no
established Turkish rendering.

Worked example, from `common/locales/tr/module_max_lines.ftl`:

```ftl
module_max_lines = { $module } modülü { $lines } satır kaplıyor ve izin verilen { $limit } sınırını aşıyor.
```

| title                | preferred              | allowed                | forbidden         |
| -------------------- | ---------------------- | ---------------------- | ----------------- |
| lint                 | `lint`                 | `lint kuralı`          | —                 |
| crate                | `crate`                | `sandık`               | `paket`           |
| module               | `modül`                | —                      | —                 |
| function             | `işlev`                | `fonksiyon`            | —                 |
| helper function      | `yardımcı işlev`       | `yardımcı fonksiyon`   | —                 |
| closure              | `kapanış`              | `closure`              | —                 |
| macro expansion      | `makro genişletmesi`   | —                      | —                 |
| attribute            | `öznitelik`            | —                      | `özellik`         |
| outer attribute      | `dış öznitelik`        | —                      | —                 |
| doc comment          | `belge yorumu`         | `dokümantasyon yorumu` | —                 |
| inner doc comment    | `iç belge yorumu`      | —                      | —                 |
| test                 | `test`                 | —                      | —                 |
| panic                | `panik`                | `panic`                | —                 |
| variant              | `varyant`              | —                      | `değişken`        |
| fallback             | `yedek`                | `geri dönüş`           | `yedekleme`       |
| cooked string        | `işlenmiş dize`        | —                      | `ham dize`        |
| escaped newline      | `kaçışlı satır sonu`   | —                      | —                 |
| cognitive complexity | `bilişsel karmaşıklık` | —                      | —                 |
| complexity bump      | `karmaşıklık tümseği`  | —                      | —                 |
| threshold            | `eşik`                 | `eşik değeri`          | `sınır`           |
| branch               | `dal`                  | —                      | —                 |
| test coverage        | `test kapsamı`         | —                      | —                 |
| working directory    | `çalışma dizini`       | —                      | `çalışma klasörü` |
| capability           | `yetenek`              | —                      | `izin`            |
| diagnostic           | `tanılama`             | `tanılama iletisi`     | —                 |

_Table 36: Turkish terminology._

### Ukrainian (`uk`)

Ukrainian diagnostics address the reader with the polite plural `ви`, written
in lower case, and `.help` uses the plural imperative (`Винесіть`, `Розділіть`,
`Перемістіть`), following the convention the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for Ukrainian software. Conditions use impersonal `-но`/`-то` forms
(`Виявлено`, `розташовано`), which are idiomatic in Ukrainian technical prose
and avoid gendering anyone. Quotation marks are `«…»`, and the apostrophe is
written as `’` (U+2019). The vocabulary is Ukrainian rather than a
transliteration of Russian: `складність`, `рядок`, `гілка`, and `замикання`.

`крейт` and `лінт` are transliterated loans that decline as masculine nouns,
consistent with the Ukrainian entry among the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html);
`модуль`, `функція`, `замикання`, `макрос`, `атрибут`, `варіант`, and `паніка`
are established terms. Identifiers and paths stay in Latin script.

Ukrainian numerals govern case (1, 2–4, 5 and above, fractions), so each
counted sentence is a four-way select. As in Russian, one word, `рядок`, means
both a line and a string; the catalogue reserves `рядок` for a line and says
`рядковий літерал` for a string, so `cooked string` becomes the coinage
`оброблюваний рядковий літерал`. `fallback` is rendered `резервна поведінка`,
never `запасний варіант`, because `варіант` is reserved for an enum variant.
`complexity bump` (`вибоїна складності`) and `capability` (`можливість`) are
coinages; `дозвіл` is avoided for `capability` because it means a permission.
English fragments sit in `«…»` after a declinable head noun
(`вираз «{ $name }»`, `елементів «{ $subject }»`). `атрибут` always means a
Rust attribute.

Worked example, from `common/locales/uk/module_max_lines.ftl`:

```ftl
module_max_lines = { $lines ->
        [one] Модуль { $module } займає { $lines } рядок, що перевищує дозволений ліміт { $limit }.
        [few] Модуль { $module } займає { $lines } рядки, що перевищує дозволений ліміт { $limit }.
        [many] Модуль { $module } займає { $lines } рядків, що перевищує дозволений ліміт { $limit }.
       *[other] Модуль { $module } займає { $lines } рядка, що перевищує дозволений ліміт { $limit }.
    }
```

| title                | preferred                             | allowed                                               | forbidden                                   |
| -------------------- | ------------------------------------- | ----------------------------------------------------- | ------------------------------------------- |
| lint                 | `лінт`                                | `lint`                                                | —                                           |
| crate                | `крейт`                               | —                                                     | `пакет` (means a Cargo package)             |
| module               | `модуль`                              | —                                                     | —                                           |
| function             | `функція`                             | —                                                     | —                                           |
| helper function      | `допоміжна функція`                   | —                                                     | —                                           |
| closure              | `замикання`                           | —                                                     | —                                           |
| macro expansion      | `розгортання макросу`                 | `розкриття макросу`                                   | —                                           |
| attribute            | `атрибут`                             | —                                                     | —                                           |
| outer attribute      | `зовнішній атрибут`                   | —                                                     | —                                           |
| doc comment          | `документаційний коментар`            | —                                                     | —                                           |
| inner doc comment    | `внутрішній документаційний коментар` | —                                                     | —                                           |
| test                 | `тест`                                | —                                                     | —                                           |
| panic                | `паніка`                              | —                                                     | —                                           |
| variant              | `варіант`                             | —                                                     | —                                           |
| fallback             | `резервна поведінка`                  | `резервний` (adjective, as in `резервна діагностика`) | `запасний варіант`                          |
| cooked string        | `оброблюваний рядковий літерал`       | —                                                     | `оброблюваний рядок`; `сирий рядок`         |
| escaped newline      | `екранований символ нового рядка`     | —                                                     | —                                           |
| cognitive complexity | `когнітивна складність`               | —                                                     | —                                           |
| complexity bump      | `вибоїна складності`                  | —                                                     | —                                           |
| threshold            | `поріг`                               | —                                                     | `ліміт` (reserved for the configured limit) |
| branch               | `гілка`                               | —                                                     | —                                           |
| test coverage        | `покриття тестами`                    | `тестове покриття`                                    | —                                           |
| working directory    | `робочий каталог`                     | `робоча директорія`                                   | `папка`                                     |
| capability           | `можливість`                          | —                                                     | `дозвіл`                                    |
| diagnostic           | `діагностика`                         | `діагностичне повідомлення`                           | —                                           |

_Table 37: Ukrainian terminology._

### Vietnamese (`vi`)

Vietnamese diagnostics use a neutral, impersonal register. Vietnamese pronouns
encode age, gender, and relative status, so the catalogue never addresses the
reader with `bạn`, `anh`, or `chị`; conditions are stated without a subject
pronoun, and instructions open with `Hãy` followed by the verb, the form the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
describe for user instructions. Prohibitions open with `Tránh`. Span labels
are lower-case noun or verb phrases without a full stop. Latin punctuation is
used, with `“ ”` as the primary quotation marks, and English fragments supplied
by the lint are enclosed in them. Vietnamese nouns do not inflect for number,
so counts are written inline (`{ $branches } nhánh`, `{ $lines } dòng`) and no
plural select is needed. Diacritics follow the classic tone-placement style
(`hoá`, `xoá`).

Vietnamese developer writing keeps `lint`, `crate`, `closure`, `macro`, `panic`,
`handle`, and `literal` as Latin-script loans, and the catalogue does the
same; identifiers are never translated. Native terms are used where they are
settled: `mô-đun`, `hàm`, `thuộc tính`, `chú thích tài liệu`, `kiểm thử`,
`biến thể`, `nhánh`, `ngưỡng`, `thư mục làm việc`, and `chẩn đoán`.

`thuộc tính` names the Rust attribute and is also the everyday word for
property, so the catalogue writes `thuộc tính ngoài` or names the attribute
through `{ $attribute }`. `kiểm thử` is the noun and verb for test;
`thử nghiệm` is avoided because it means experiment. `nhánh` renders branch and
`rẽ nhánh` renders branching. `phương án dự phòng` renders fallback; `dự phòng`
alone is used attributively. `năng lực` renders capability and is kept distinct
from `quyền`, which means permission. Coinages with no established Vietnamese
rendering: `chuỗi không thô` for cooked string (by contrast with `chuỗi thô`,
raw string), `gờ phức tạp` for complexity bump, `chú thích tài liệu trong` for
inner doc comment, and `độ phức tạp nhận thức` for cognitive complexity.

Worked example, from `common/locales/vi/module_max_lines.ftl`:

```ftl
module_max_lines = Mô-đun { $module } dài { $lines } dòng, vượt quá mức cho phép là { $limit } dòng.
```

| title                | preferred                     | allowed                  | forbidden                                      |
| -------------------- | ----------------------------- | ------------------------ | ---------------------------------------------- |
| lint                 | `lint`                        | —                        | —                                              |
| crate                | `crate`                       | —                        | `gói` (a Cargo package is a different concept) |
| module               | `mô-đun`                      | `module`                 | —                                              |
| function             | `hàm`                         | —                        | —                                              |
| helper function      | `hàm trợ giúp`                | `hàm hỗ trợ`             | —                                              |
| closure              | `closure`                     | `bao đóng`               | —                                              |
| macro expansion      | `khai triển macro`            | `mở rộng macro`          | —                                              |
| attribute            | `thuộc tính`                  | —                        | —                                              |
| outer attribute      | `thuộc tính ngoài`            | —                        | —                                              |
| doc comment          | `chú thích tài liệu`          | `doc comment`            | —                                              |
| inner doc comment    | `chú thích tài liệu trong`    | —                        | —                                              |
| test                 | `kiểm thử`                    | —                        | `thử nghiệm` (experiment)                      |
| panic                | `panic`                       | —                        | —                                              |
| variant              | `biến thể`                    | —                        | `biến` (means variable)                        |
| fallback             | `phương án dự phòng`          | `dự phòng` (attributive) | —                                              |
| cooked string        | `chuỗi không thô`             | `chuỗi thông thường`     | `chuỗi thô` (means raw string)                 |
| escaped newline      | `ký tự xuống dòng được thoát` | —                        | —                                              |
| cognitive complexity | `độ phức tạp nhận thức`       | —                        | `độ phức tạp vòng` (cyclomatic complexity)     |
| complexity bump      | `gờ phức tạp`                 | —                        | —                                              |
| threshold            | `ngưỡng`                      | —                        | —                                              |
| branch               | `nhánh`                       | —                        | —                                              |
| test coverage        | `độ bao phủ kiểm thử`         | `độ phủ kiểm thử`        | —                                              |
| working directory    | `thư mục làm việc`            | `thư mục hiện hành`      | —                                              |
| capability           | `năng lực`                    | `capability`             | `quyền` (permission)                           |
| diagnostic           | `chẩn đoán`                   | `thông báo chẩn đoán`    | —                                              |

_Table 38: Vietnamese terminology._

### Chinese, Simplified (`zh-Hans`)

Simplified Chinese diagnostics are written in Simplified script with Mainland
technical vocabulary; `zh-Hans` and `zh-Hant` differ in vocabulary as well as
script (for example `函数`, `模块`, `文档`, `宏`, `注释`, `字符串`, and `默认`
here against `函式`, `模組`, `文件`, `巨集`, `註解`, `字串`, and `預設` there),
so neither catalogue may be produced by character conversion of the other. The
register is neutral written Chinese: conditions are plain statements, and
instructions open with `请` followed by the verb, as the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
recommend; the reader is never addressed as `你` or `您`. Span labels are
short noun or verb phrases without `。`. Full-width punctuation (`，`, `。`,
`；`, `：`, `（）`, `“”`) is used in prose, and one half-width space separates
Han characters from placeables, code spans, Latin identifiers, and numbers,
except next to full-width punctuation. English fragments supplied by the lint
are enclosed in `“”`. Counts take a measure word (`{ $branches } 个分支`,
`{ $lines } 行`), so no plural select is needed.

Following the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html),
`lint`, `crate`, and `panic` stay as Latin-script loans, as do all identifiers.
The remaining vocabulary uses established native terms: `模块`, `函数`, `闭包`,
`宏展开`, `属性`, `文档注释`, `测试`, `变体`, `分支`, `阈值`, and `测试覆盖率`.

`crate` is not translated because `包` means a Cargo package. `属性` names the
Rust attribute only; a Fluent attribute never appears in rendered text. `回退`
renders fallback and must not become `回滚`, which means rollback. `测试`
serves as noun and verb; the catalogue uses it as a noun. `分支` covers both
branch and branching. `能力` renders capability and is always qualified
(`基于能力的`, `能力范围`) so that it does not read as general ability; `权限`
is avoided because it means permission. Coinages with no established rendering:
`非原始字符串` for cooked string (by contrast with `原始字符串`, raw string),
`复杂度凸起` for complexity bump, and `内部文档注释` for inner doc comment.

Worked example, from `common/locales/zh-Hans/module_max_lines.ftl`:

```ftl
module_max_lines = 模块 { $module } 共 { $lines } 行，超过了允许的 { $limit } 行。
```

| title                | preferred      | allowed            | forbidden                                     |
| -------------------- | -------------- | ------------------ | --------------------------------------------- |
| lint                 | `lint`         | `lint 检查`        | —                                             |
| crate                | `crate`        | —                  | `包` (a Cargo package is a different concept) |
| module               | `模块`         | —                  | `模組` (Traditional form)                     |
| function             | `函数`         | —                  | `函式` (Traditional form)                     |
| helper function      | `辅助函数`     | `帮助函数`         | —                                             |
| closure              | `闭包`         | —                  | —                                             |
| macro expansion      | `宏展开`       | —                  | `巨集展開` (Traditional form)                 |
| attribute            | `属性`         | —                  | —                                             |
| outer attribute      | `外部属性`     | —                  | —                                             |
| doc comment          | `文档注释`     | —                  | `文件註解` (Traditional form)                 |
| inner doc comment    | `内部文档注释` | —                  | —                                             |
| test                 | `测试`         | —                  | —                                             |
| panic                | `panic`        | `恐慌`             | —                                             |
| variant              | `变体`         | `成员`             | —                                             |
| fallback             | `回退`         | `后备`             | `回滚` (rollback)                             |
| cooked string        | `非原始字符串` | `普通字符串字面量` | `原始字符串` (means raw string)               |
| escaped newline      | `转义换行符`   | `转义的换行`       | —                                             |
| cognitive complexity | `认知复杂度`   | —                  | `圈复杂度` (cyclomatic complexity)            |
| complexity bump      | `复杂度凸起`   | —                  | —                                             |
| threshold            | `阈值`         | —                  | —                                             |
| branch               | `分支`         | —                  | —                                             |
| test coverage        | `测试覆盖率`   | —                  | —                                             |
| working directory    | `工作目录`     | `当前目录`         | —                                             |
| capability           | `能力`         | —                  | `权限` (permission)                           |
| diagnostic           | `诊断信息`     | `诊断`             | —                                             |

_Table 39: Chinese, Simplified terminology._

### Chinese, Traditional (`zh-Hant`)

Traditional Chinese diagnostics are written in Traditional script with Taiwan
technical vocabulary; `zh-Hant` and `zh-Hans` differ in vocabulary as well as
script (for example `函式`, `模組`, `文件`, `巨集`, `註解`, `字串`, `程式碼`,
`呼叫`, and `預設` here against `函数`, `模块`, `文档`, `宏`, `注释`, `字符串`,
`代码`, `调用`, and `默认` there), so neither catalogue may be produced by
character conversion of the other. The register is neutral written Chinese:
conditions are plain statements, and instructions open with `請` followed by
the verb, as the
[Microsoft localization style guides](https://learn.microsoft.com/globalization/reference/microsoft-style-guides)
recommend; the reader is never addressed as `你` or `您`. Span labels are
short noun or verb phrases without `。`. Full-width punctuation (`，`, `。`,
`；`, `：`, `（）`) is used in prose with corner brackets `「」` as the primary
quotation marks, and one half-width space separates Han characters from
placeables, code spans, Latin identifiers, and numbers, except next to
full-width punctuation. English fragments supplied by the lint are enclosed in
`「」`. Counts take a measure word (`{ $branches } 個分支`, `{ $lines } 行`),
so no plural select is needed.

Following the
[community translations of _The Rust Programming Language_](https://doc.rust-lang.org/book/appendix-06-translation.html),
`lint`, `crate`, and `panic` stay as Latin-script loans, as do all identifiers.
The remaining vocabulary uses established Taiwan terms: `模組`, `函式`, `閉包`,
`巨集展開`, `屬性`, `文件註解`, `測試`, `變體`, `分支`, `閾值`, and
`測試涵蓋率`.

`crate` is not translated because `套件` means a Cargo package. `文件` means
documentation in Taiwan usage, whereas the same characters mean a file in
Mainland usage; a file is `檔案` here. `屬性` names the Rust attribute only.
`後援` renders fallback, following Taiwan software usage, and must not become
`回復`, which means restore. `能力` renders capability and is always qualified
(`以能力為基礎的`, `能力範圍`); `權限` is avoided because it means permission.
Coinages with no established rendering: `非原始字串` for cooked string (by
contrast with `原始字串`, raw string), `複雜度突起` for complexity bump, and
`內部文件註解` for inner doc comment.

Worked example, from `common/locales/zh-Hant/module_max_lines.ftl`:

```ftl
module_max_lines = 模組 { $module } 共 { $lines } 行，超過允許的 { $limit } 行。
```

| title                | preferred      | allowed        | forbidden                                       |
| -------------------- | -------------- | -------------- | ----------------------------------------------- |
| lint                 | `lint`         | `lint 檢查`    | —                                               |
| crate                | `crate`        | —              | `套件` (a Cargo package is a different concept) |
| module               | `模組`         | —              | `模块` (Simplified form)                        |
| function             | `函式`         | —              | `函数` (Simplified form)                        |
| helper function      | `輔助函式`     | —              | —                                               |
| closure              | `閉包`         | —              | —                                               |
| macro expansion      | `巨集展開`     | —              | `宏展开` (Simplified form)                      |
| attribute            | `屬性`         | —              | —                                               |
| outer attribute      | `外部屬性`     | —              | —                                               |
| doc comment          | `文件註解`     | `說明文件註解` | `文档注释` (Simplified form)                    |
| inner doc comment    | `內部文件註解` | —              | —                                               |
| test                 | `測試`         | —              | —                                               |
| panic                | `panic`        | `恐慌`         | —                                               |
| variant              | `變體`         | —              | —                                               |
| fallback             | `後援`         | `備援`         | `回復` (restore)                                |
| cooked string        | `非原始字串`   | `一般字串常值` | `原始字串` (means raw string)                   |
| escaped newline      | `跳脫換行字元` | `跳脫的換行`   | —                                               |
| cognitive complexity | `認知複雜度`   | —              | `循環複雜度` (cyclomatic complexity)            |
| complexity bump      | `複雜度突起`   | —              | —                                               |
| threshold            | `閾值`         | `臨界值`       | —                                               |
| branch               | `分支`         | —              | —                                               |
| test coverage        | `測試涵蓋率`   | `測試覆蓋率`   | —                                               |
| working directory    | `工作目錄`     | `目前目錄`     | —                                               |
| capability           | `能力`         | —              | `權限` (permission)                             |
| diagnostic           | `診斷訊息`     | `診斷`         | —                                               |

_Table 40: Chinese, Traditional terminology._

## Maintenance

A terminology change is a change to shipped diagnostics, so it travels with the
catalogue edit that applies it:

1. Change the term in the locale's table, and explain the reason in the prose
   above the table.
2. Update every message in `common/locales/<tag>/` that uses the old form, in
   the same pull request, so the table and the catalogue never disagree.
3. If the change alters a source term, update the source tables first, then
   review each locale section for a mapping that the change invalidates.
4. Re-bless any locale-specific UI fixture that renders the changed message.
   The [translator's guide](translators-guide.md) describes those fixtures and
   the gates to run.

When a fluent reader reviews a machine-drafted section, record the review in
that section's prose: who reviewed it, when, and which rows changed. Remove the
locale from the list of unreviewed sections above only when the whole table has
been reviewed. A locale that is planned but not yet shipped does not belong in
this document until its catalogue exists.
