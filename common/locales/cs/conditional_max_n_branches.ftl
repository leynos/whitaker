## Podmíněné výrazy mají zůstat mělké.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Zredukujte výraz „{ $name }“ nejvýše na { $limit } větev.
        [few] Zredukujte výraz „{ $name }“ nejvýše na { $limit } větve.
        [many] Zredukujte výraz „{ $name }“ nejvýše na { $limit } větve.
       *[other] Zredukujte výraz „{ $name }“ nejvýše na { $limit } větví.
    }
    .note = { $branches ->
        [one] Výraz „{ $name }“ nyní obsahuje { $branches } větev.
        [few] Výraz „{ $name }“ nyní obsahuje { $branches } větve.
        [many] Výraz „{ $name }“ nyní obsahuje { $branches } větve.
       *[other] Výraz „{ $name }“ nyní obsahuje { $branches } větví.
    }
    .help = Extrahujte pomocné funkce nebo výraz „{ $name }“ zjednodušte, aby se větvení omezilo.
