## As condicionais devem permanecer pouco profundas.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reduza “{ $name }” a no máximo { $limit } ramificação.
       *[other] Reduza “{ $name }” a no máximo { $limit } ramificações.
    }
    .note = { $branches ->
        [one] Atualmente, “{ $name }” contém { $branches } ramificação.
       *[other] Atualmente, “{ $name }” contém { $branches } ramificações.
    }
    .help = Extraia funções auxiliares ou simplifique “{ $name }” para reduzir as ramificações.
