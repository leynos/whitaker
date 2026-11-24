## Le condizioni devono restare poco profonde.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Ridurre «{ $name }» a non più di { $limit } ramo.
       *[other] Ridurre «{ $name }» a non più di { $limit } rami.
    }
    .note = { $branches ->
        [one] Attualmente «{ $name }» contiene { $branches } ramo.
       *[other] Attualmente «{ $name }» contiene { $branches } rami.
    }
    .help = Estrarre funzioni ausiliarie o semplificare «{ $name }» per ridurre le diramazioni.
