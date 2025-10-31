## Cynorthwywyr diagnostig a rennir ar draws linterau Whitaker.

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = modiwl
-term-branch = cangen
-term-test-coverage = cwmpas profion

common-lint-count =
    { $lint ->
        [0] Cyfrif { -term-lint }: dim lint.
        [1] Cyfrif { -term-lint }: un lint.
        [2] Cyfrif { -term-lint }: dau lint.
        [3] Cyfrif { -term-lint }: tri lint.
       *[other] Cyfrif { -term-lint }: { $lint } lint.
    }
    .note = Mae negeseuon ar gael yn Gymraeg ar gyfer { $lint }.
    .help = Ychwanegu cyfieithiadau ar gyfer pob lint i gadw cwmpas profion yn gyflawn.
    .fallback-note = Mae'r negeseuon wrth gefn yn parhau yn Saesneg.

#. Shown in diagnostics when referring to the preceding attribute name.
common-attribute-fallback = y briodoledd flaenorol
