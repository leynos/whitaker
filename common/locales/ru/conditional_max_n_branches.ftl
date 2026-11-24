## Условные выражения должны оставаться неглубокими.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Сократите выражение «{ $name }» не более чем до { $limit } ветви.
        [few] Сократите выражение «{ $name }» не более чем до { $limit } ветвей.
        [many] Сократите выражение «{ $name }» не более чем до { $limit } ветвей.
       *[other] Сократите выражение «{ $name }» не более чем до { $limit } ветви.
    }
    .note = { $branches ->
        [one] Сейчас выражение «{ $name }» содержит { $branches } ветвь.
        [few] Сейчас выражение «{ $name }» содержит { $branches } ветви.
        [many] Сейчас выражение «{ $name }» содержит { $branches } ветвей.
       *[other] Сейчас выражение «{ $name }» содержит { $branches } ветви.
    }
    .help = Вынесите вспомогательные функции или упростите выражение «{ $name }», чтобы уменьшить ветвление.
