## Voorwaarden moeten ondiep blijven.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Breng “{ $name }” terug tot hooguit { $limit } tak.
       *[other] Breng “{ $name }” terug tot hooguit { $limit } takken.
    }
    .note = { $branches ->
        [one] “{ $name }” bevat momenteel { $branches } tak.
       *[other] “{ $name }” bevat momenteel { $branches } takken.
    }
    .help = Verplaats logica naar hulpfuncties of vereenvoudig “{ $name }” om de vertakking te verminderen.
