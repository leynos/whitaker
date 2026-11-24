## Pokyny k délce modulu.

module_max_lines = { $lines ->
        [one] Modul { $module } má { $lines } řádek, což překračuje povolený limit { $limit }.
        [few] Modul { $module } má { $lines } řádky, což překračuje povolený limit { $limit }.
        [many] Modul { $module } má { $lines } řádku, což překračuje povolený limit { $limit }.
       *[other] Modul { $module } má { $lines } řádků, což překračuje povolený limit { $limit }.
    }
    .note = Rozsáhlé moduly se hůře procházejí a revidují.
    .help = Rozdělte modul { $module } na menší moduly nebo omezte jeho odpovědnosti.
