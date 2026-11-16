## Betingelser bør forblive flade.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reducer “{ $name }” til højst { $limit } gren.
       *[other] Reducer “{ $name }” til højst { $limit } grene.
    }
    .note = { $branches ->
        [one] “{ $name }” indeholder i øjeblikket { $branches } gren.
       *[other] “{ $name }” indeholder i øjeblikket { $branches } grene.
    }
    .help = Udtræk hjælpefunktioner, eller forenkl “{ $name }” for at reducere forgreningen.
