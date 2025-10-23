## Cynorthwywyr diagnostig a rennir ar draws linterau Whitaker.

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = modiwl
-term-branch = cangen
-term-test-coverage = cwmpas profion
-term-lint-count =
    { $lint ->
        [zero] dim { -term-lint }
        [one] { $lint } { -term-lint }
        [two] dau { -term-lint }
        [few] { $lint } { -term-lint }
        [many] { $lint } { -term-lint }
       *[other] { $lint } { -term-lint }
    }

common-lint-count = Cyfrif { -term-lint-count(lint: $lint) }.
    .note = Mae negeseuon ar gael yn Gymraeg ar gyfer { -term-lint-count(lint: $lint) }.
    .help = Ychwanegu cyfieithiadau ar gyfer pob { -term-lint-count(lint: $lint) } i gadw cwmpas profion yn gyflawn.
