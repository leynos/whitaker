## Cynorthwywyr diagnostig a rennir ar draws linterau Whitaker.

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = modiwl
-term-branch = cangen
-term-test-coverage = cwmpas profion

# Borrowed English nouns typically pluralise with -iau (Modern Welsh, Gareth
# King §2.8), so we render "lint" as "lintiau" in aggregated messaging.
common-lint-count = Cyfrif lintiau: { $lint }.
    .note = Mae negeseuon ar gael yn Gymraeg ar gyfer { $lint } o lintiau.
    .help =
        Ychwanegu cyfieithiadau ar gyfer pob lint i gadw cwmpas profion yn
        gyflawn.
    .fallback-note = Mae diagnosteg wrth gefn yn ddiofyn i'r Saesneg.

#. Shown in diagnostics when referring to the preceding attribute name.
common-attribute-fallback = y briodoledd flaenorol
