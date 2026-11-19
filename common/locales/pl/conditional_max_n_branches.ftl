## Wyrażenia warunkowe powinny pozostać płytkie.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Ogranicz wyrażenie „{ $name }” do najwyżej { $limit } gałęzi.
        [few] Ogranicz wyrażenie „{ $name }” do najwyżej { $limit } gałęzi.
        [many] Ogranicz wyrażenie „{ $name }” do najwyżej { $limit } gałęzi.
       *[other] Ogranicz wyrażenie „{ $name }” do najwyżej { $limit } gałęzi.
    }
    .note = { $branches ->
        [one] Wyrażenie „{ $name }” zawiera obecnie { $branches } gałąź.
        [few] Wyrażenie „{ $name }” zawiera obecnie { $branches } gałęzie.
        [many] Wyrażenie „{ $name }” zawiera obecnie { $branches } gałęzi.
       *[other] Wyrażenie „{ $name }” zawiera obecnie { $branches } gałęzi.
    }
    .help = Wyodrębnij funkcje pomocnicze lub uprość wyrażenie „{ $name }”, aby ograniczyć rozgałęzianie.
