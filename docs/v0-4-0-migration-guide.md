# Migration guide: v0.3.0 to v0.4.0

## Who should read this

Read this guide if a workspace or environment selects a diagnostic locale
through `DYLINT_LOCALE` or the `locale` key in `dylint.toml`. Whitaker 0.4.0
ships diagnostic catalogues for 35 locales, up from `en-GB`, `cy`, and `gd`.
Whitaker uses the first locale setting, `DYLINT_LOCALE` and then `dylint.toml`,
that names a shipped tag, and `en-GB` when neither does. Earlier releases
passed over a setting that named one of the new tags, usually ending in English
output. Where such a setting is now the first one naming a shipped tag, it
selects that language.

## Impact at a glance

| Priority            | Area                  | What to do                                                                                    |
| ------------------- | --------------------- | --------------------------------------------------------------------------------------------- |
| Review if enabled   | Diagnostic locale     | Check whether `DYLINT_LOCALE` or `locale` names one of the new tags, and keep or change it.   |
| Required if matched | Scripts and fixtures  | Pin `en-GB` where a script or test fixture matches the English text of a Whitaker diagnostic. |
| Review if opting in | New experimental lint | Decide whether to enable `missing_docs_in_test_functions`; no action if you do not opt in.    |

_Table 1: Obligations when moving from v0.3.0 to v0.4.0._

## 1. Check the configured locale

The [translator's guide](translators-guide.md#shipped-locales) lists every
shipped tag. Find the first setting, `DYLINT_LOCALE` and then `dylint.toml`,
that names one of them. If that tag is not `en-GB`, `cy`, or `gd`, diagnostics
now render in its language where they used to fall through, usually to English.
A lower-priority setting that names a new tag changes nothing while a
higher-priority one names `en-GB`, `cy`, or `gd`. Keep the setting to adopt the
translation. The new catalogues were machine-drafted and await review by a
fluent reader, as the
[localization glossary](localization-glossary.md#locale-terminology) records.

## 2. Pin English where text is matched

Set the locale to `en-GB` wherever a script, test, or golden file compares a
Whitaker diagnostic's text with English, so the comparison keeps seeing English.

The examples below change `dylint.toml`, so they assume `DYLINT_LOCALE` is
unset or set to `en-GB`. A `DYLINT_LOCALE` naming another shipped tag, such as
`de`, takes precedence over the file and keeps that language.

### Before

```toml
# dylint.toml: ignored by 0.3.0, so diagnostics printed in English.
locale = "de"
```

### After

```toml
# dylint.toml: keeps English output on 0.4.0.
locale = "en-GB"
```

The same applies to the environment: replace `DYLINT_LOCALE=de` with
`DYLINT_LOCALE=en-GB`. Unsetting the variable instead hands the choice to
`dylint.toml`: the output is English only when the file names `en-GB` or no
shipped tag, and is in whatever other shipped language the file names otherwise.

One new lint ships in 0.4.0 and needs no migration work.
`missing_docs_in_test_functions` is experimental, so a repository must list it
under `[whitaker] experimental` in `dylint.toml` before it runs at all; every
repository that does not is unaffected. Section 3 covers what an opting-in
repository should know.

## 3. New experimental lint: `missing_docs_in_test_functions`

The suite gains a pre-expansion lint that reports undocumented functions in a
compilation built for a test harness. It is experimental under
[ADR 0014](adrs/adr-0014-per-lint-experimental-opt-in.md): the prebuilt suite
declares it, but nothing runs unless the repository opts in.

### Opting in

Before, the lint is declared and inert:

```toml
# dylint.toml: no experimental entry, so nothing runs.
```

After, it warns on undocumented test functions:

```toml
# dylint.toml: opt in.
[whitaker]
experimental = ["missing_docs_in_test_functions"]
```

Scope is wider than the name suggests. Until the follow-on include and exclude
patterns ship, the path gate selects every real source file, so an undocumented
private helper in an ordinary library source file is reported too, provided the
compilation is a test harness. Do not assume the lint is confined to `#[test]`
modules or to `tests/`: a library target compiled for tests reports its own
helpers. The user's guide records this as interim behaviour.

Three consequences surprise a first-time user:

- The lint needs test targets. With `cargo dylint`, pass `--all-targets` or
  `--tests` after `--`, as in `cargo dylint --all -- --all-targets`. A plain
  `cargo check` builds no test targets, so the lint never fires.
- `#[expect(missing_docs_in_test_functions)]` misfires on functions carrying a
  procedural attribute macro such as `#[given]` or `#[rstest]`. Use the
  two-attribute `allow` workaround the
  [user's guide](users-guide.md#missing_docs_in_test_functions) documents.
- There is no logging channel to explain a silent run. The user's guide lists
  the gates to check instead.

### What opting in does not change

Repositories that do not opt in see no diagnostics, no new warnings, and no
change to any existing lint's output.

The lint's default level is `warn`, so opting in adds warnings rather than
errors — unless the repository raises the level. A workspace that sets
`missing_docs_in_test_functions = "deny"` in `[workspace.lints.rust]`, or passes
`-D missing_docs_in_test_functions`, turns every one of these diagnostics into
a build failure, and `-D warnings` does the same. Decide the level before
opting in, particularly in CI where a new warning can fail a build that
previously passed.

## What does not need to change

Configurations whose locale settings name only `en-GB`, `cy`, or `gd`, or that
set no locale, behave as before. Lint names, `#[allow(...)]` and
`#[expect(...)]` attributes, severity levels, spans, and the compiler's own
`warning`, `note`, and `help` labels do not change in any locale. A tag that is
still not shipped, such as `fr-CA` or a bare `es`, is skipped as before, so the
next setting applies and `en-GB` is used only when no setting names a shipped
tag; the [user's guide](users-guide.md#localized-diagnostics) explains how to
request a regional catalogue.

## Migration checklist

- [ ] Find every `DYLINT_LOCALE` and `locale` setting the workspace, CI, and
      editor integrations use.
- [ ] For each that names a newly shipped tag, decide whether to keep the
      translation or pin `en-GB`.
- [ ] Pin `en-GB` for every script or fixture that matches English diagnostic
      text.
- [ ] Decide whether to opt in to `missing_docs_in_test_functions`, and tell
      contributors which suppression form to use if you do.

## Common errors and fixes

- **Error:** a golden-file or text-matching test that expected this message:

  ```plaintext
  Module long_module spans 18 lines, exceeding the allowed 5.
  ```

  now reports a translated message, such as:

  ```plaintext
  Das Modul long_module umfasst 18 Zeilen und überschreitet den zulässigen Grenzwert von 5.
  ```

  - **Fix:** set `DYLINT_LOCALE=en-GB` for that test, as section 2 shows.
