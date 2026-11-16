## सशर्त अभिव्यक्तियाँ उथली रहनी चाहिए।

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] “{ $name }” को अधिकतम { $limit } शाखा तक सीमित करें।
       *[other] “{ $name }” को अधिकतम { $limit } शाखाओं तक सीमित करें।
    }
    .note = { $branches ->
        [one] “{ $name }” में अभी { $branches } शाखा है।
       *[other] “{ $name }” में अभी { $branches } शाखाएँ हैं।
    }
    .help = शाखाएँ घटाने के लिए सहायक फ़ंक्शन निकालें या “{ $name }” को सरल बनाएँ।
