## Ehtojen tulee pysyä matalina.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Supista ”{ $name }” enintään { $limit } haaraan.
       *[other] Supista ”{ $name }” enintään { $limit } haaraan.
    }
    .note = { $branches ->
        [one] ”{ $name }” sisältää tällä hetkellä { $branches } haaran.
       *[other] ”{ $name }” sisältää tällä hetkellä { $branches } haaraa.
    }
    .help = Erota logiikkaa apufunktioiksi tai yksinkertaista kohdetta ”{ $name }” haarautumisen vähentämiseksi.
