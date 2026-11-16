## Las condiciones deben seguir siendo poco profundas.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reduce «{ $name }» a { $limit } rama como máximo.
       *[other] Reduce «{ $name }» a { $limit } ramas como máximo.
    }
    .note = { $branches ->
        [one] Actualmente, «{ $name }» contiene { $branches } rama.
       *[other] Actualmente, «{ $name }» contiene { $branches } ramas.
    }
    .help = Extrae funciones auxiliares o simplifica «{ $name }» para reducir la ramificación.
