## Richtlijnen voor modulelengte.

module_max_lines = { $lines ->
        [one] Module { $module } beslaat { $lines } regel en overschrijdt daarmee de toegestane limiet van { $limit }.
       *[other] Module { $module } beslaat { $lines } regels en overschrijdt daarmee de toegestane limiet van { $limit }.
    }
    .note = Grote modules zijn moeilijker te doorzoeken en te beoordelen.
    .help = Splits { $module } op in kleinere modules of beperk de verantwoordelijkheden ervan.
