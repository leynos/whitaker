## Common diagnostic helpers shared across lint crates.

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = module
-term-branch = branch
-term-test-coverage = test coverage

common-lint-count = Count your lints: { $lint }.
    .note = Messages for { $lint } are available in this locale.
    .help = Add translations for every lint slug to keep test coverage intact.
    .fallback-note = Fallback diagnostics default to English.

#. Shown in diagnostics to refer to the preceding attribute name.
common-attribute-fallback = the preceding attribute
