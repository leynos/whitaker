## Villkor bör hållas grunda.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Minska ”{ $name }” till högst { $limit } gren.
       *[other] Minska ”{ $name }” till högst { $limit } grenar.
    }
    .note = { $branches ->
        [one] ”{ $name }” innehåller för närvarande { $branches } gren.
       *[other] ”{ $name }” innehåller för närvarande { $branches } grenar.
    }
    .help = Bryt ut hjälpfunktioner eller förenkla ”{ $name }” för att minska förgreningen.
