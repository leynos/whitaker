## شرط‌ها باید کم‌عمق بمانند.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] «{ $name }» را به حداکثر { $limit } شاخه کاهش دهید.
       *[other] «{ $name }» را به حداکثر { $limit } شاخه کاهش دهید.
    }
    .note = { $branches ->
        [one] «{ $name }» اکنون { $branches } شاخه دارد.
       *[other] «{ $name }» اکنون { $branches } شاخه دارد.
    }
    .help = برای کاهش شاخه‌ها، تابع‌های کمکی استخراج کنید یا «{ $name }» را ساده کنید.
