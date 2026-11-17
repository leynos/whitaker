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

| Priority            | Area                 | What to do                                                                                    |
| ------------------- | -------------------- | --------------------------------------------------------------------------------------------- |
| Review if enabled   | Diagnostic locale    | Check whether `DYLINT_LOCALE` or `locale` names one of the new tags, and keep or change it.   |
| Required if matched | Scripts and fixtures | Pin `en-GB` where a script or test fixture matches the English text of a Whitaker diagnostic. |

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
