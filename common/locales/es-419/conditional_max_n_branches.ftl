## Las condiciones deben mantenerse poco profundas.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reduzca “{ $name }” a { $limit } rama como máximo.
       *[other] Reduzca “{ $name }” a { $limit } ramas como máximo.
    }
    .note = { $branches ->
        [one] Actualmente, “{ $name }” contiene { $branches } rama.
       *[other] Actualmente, “{ $name }” contiene { $branches } ramas.
    }
    .help = Extraiga funciones auxiliares o simplifique “{ $name }” para reducir la ramificación.
