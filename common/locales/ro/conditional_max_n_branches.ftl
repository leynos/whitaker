## Condițiile trebuie să rămână puțin adânci.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Reduceți „{ $name }” la cel mult { $limit } ramură.
        [few] Reduceți „{ $name }” la cel mult { $limit } ramuri.
       *[other] Reduceți „{ $name }” la cel mult { $limit } de ramuri.
    }
    .note = { $branches ->
        [one] În prezent, „{ $name }” conține { $branches } ramură.
        [few] În prezent, „{ $name }” conține { $branches } ramuri.
       *[other] În prezent, „{ $name }” conține { $branches } de ramuri.
    }
    .help = Extrageți funcții auxiliare sau simplificați „{ $name }” pentru a reduce ramificarea.
