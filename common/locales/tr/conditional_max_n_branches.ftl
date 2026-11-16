## Koşullu ifadeler sığ kalmalıdır.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] “{ $name }” ifadesini en çok { $limit } dala indirin.
       *[other] “{ $name }” ifadesini en çok { $limit } dala indirin.
    }
    .note = { $branches ->
        [one] “{ $name }” ifadesi şu anda { $branches } dal içeriyor.
       *[other] “{ $name }” ifadesi şu anda { $branches } dal içeriyor.
    }
    .help = Dallanmayı azaltmak için yardımcı işlevler ayıklayın veya “{ $name }” ifadesini sadeleştirin.
