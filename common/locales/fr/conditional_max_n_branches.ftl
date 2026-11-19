## Les conditions doivent rester peu profondes.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Réduisez « { $name } » à { $limit } branche au maximum.
       *[other] Réduisez « { $name } » à { $limit } branches au maximum.
    }
    .note = { $branches ->
        [one] Actuellement, « { $name } » contient { $branches } branche.
       *[other] Actuellement, « { $name } » contient { $branches } branches.
    }
    .help = Extrayez des fonctions auxiliaires ou simplifiez « { $name } » pour réduire les embranchements.
