## Betingelser bør holdes flate.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reduser «{ $name }» til høyst { $limit } gren.
       *[other] Reduser «{ $name }» til høyst { $limit } grener.
    }
    .note = { $branches ->
        [one] «{ $name }» inneholder for øyeblikket { $branches } gren.
       *[other] «{ $name }» inneholder for øyeblikket { $branches } grener.
    }
    .help = Trekk ut hjelpefunksjoner, eller forenkle «{ $name }» for å redusere forgreningen.
