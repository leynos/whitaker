# Migration guide: v0.3.0 to v0.3.1

## Who should read this

Read this guide if a workspace or environment selects a diagnostic locale
through `DYLINT_LOCALE` or the `locale` key in `dylint.toml`. Whitaker 0.3.1
ships diagnostic catalogues for 35 locales, up from `en-GB`, `cy`, and `gd`.
Earlier releases ignored any other tag and printed English. A setting that
names one of the new tags now selects that language.

## Impact at a glance

| Priority            | Area                 | What to do                                                                                    |
| ------------------- | -------------------- | --------------------------------------------------------------------------------------------- |
| Review if enabled   | Diagnostic locale    | Check whether `DYLINT_LOCALE` or `locale` names one of the new tags, and keep or change it.   |
| Required if matched | Scripts and fixtures | Pin `en-GB` where a script or test fixture matches the English text of a Whitaker diagnostic. |

_Table 1: Obligations when moving from v0.3.0 to v0.3.1._

## 1. Check the configured locale

The [translator's guide](translators-guide.md#shipped-locales) lists every
shipped tag. If the configured value is one of them, other than `en-GB`, `cy`,
or `gd`, diagnostics now render in that language. Keep the setting to adopt the
translation. The new catalogues were machine-drafted and await review by a
fluent reader, as the
[localization glossary](localization-glossary.md#locale-terminology) records.

## 2. Pin English where text is matched

Set the locale to `en-GB` wherever a script, test, or golden file compares a
Whitaker diagnostic's text with English, so the comparison keeps seeing English.

### Before

```toml
# dylint.toml: ignored by 0.3.0, so diagnostics printed in English.
locale = "de"
```

### After

```toml
# dylint.toml: keeps English output on 0.3.1.
locale = "en-GB"
```

The same applies to the environment: replace `DYLINT_LOCALE=de` with
`DYLINT_LOCALE=en-GB`, or unset the variable.

## What does not need to change

Configurations that name `en-GB`, `cy`, or `gd`, or that set no locale, behave
as before. Lint names, `#[allow(...)]` and `#[expect(...)]` attributes,
severity levels, spans, and the compiler's own `warning`, `note`, and `help`
labels do not change in any locale. A tag that is still not shipped, such as
`fr-CA` or a bare `es`, falls back to `en-GB` as before; the
[user's guide](users-guide.md#localized-diagnostics) explains how to request a
regional catalogue.

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
