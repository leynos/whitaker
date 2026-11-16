## Bedingungen sollen flach bleiben.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reduzieren Sie „{ $name }“ auf höchstens { $limit } Zweig.
       *[other] Reduzieren Sie „{ $name }“ auf höchstens { $limit } Zweige.
    }
    .note = { $branches ->
        [one] „{ $name }“ enthält derzeit { $branches } Zweig.
       *[other] „{ $name }“ enthält derzeit { $branches } Zweige.
    }
    .help = Lagern Sie Logik in Hilfsfunktionen aus oder vereinfachen Sie „{ $name }“, um die Verzweigung zu verringern.
