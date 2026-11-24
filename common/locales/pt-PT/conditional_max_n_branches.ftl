## As condições devem manter-se pouco profundas.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reduzir «{ $name }» a um máximo de { $limit } ramo.
       *[other] Reduzir «{ $name }» a um máximo de { $limit } ramos.
    }
    .note = { $branches ->
        [one] Atualmente, «{ $name }» contém { $branches } ramo.
       *[other] Atualmente, «{ $name }» contém { $branches } ramos.
    }
    .help = Extrair funções auxiliares ou simplificar «{ $name }» para reduzir a ramificação.
