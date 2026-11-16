## A feltételeknek sekélynek kell maradniuk.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Egyszerűsítse a(z) „{ $name }” kifejezést legfeljebb { $limit } ágra.
       *[other] Egyszerűsítse a(z) „{ $name }” kifejezést legfeljebb { $limit } ágra.
    }
    .note = { $branches ->
        [one] A(z) „{ $name }” kifejezés jelenleg { $branches } ágat tartalmaz.
       *[other] A(z) „{ $name }” kifejezés jelenleg { $branches } ágat tartalmaz.
    }
    .help = Emeljen ki segédfüggvényeket, vagy egyszerűsítse a(z) „{ $name }” kifejezést az elágazások csökkentéséhez.
