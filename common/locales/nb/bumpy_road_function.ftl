## Detektor for «Bumpy Road»-kompleksitet.

bumpy_road_function = Flere klynger av nøstet betinget logikk i `{ $name }`.
    .note = { $count ->
        [one] Fant { $count } kompleksitetstopp over terskelverdien { $threshold }.
       *[other] Fant { $count } kompleksitetstopper over terskelverdien { $threshold }.
    }
    .help = Trekk ut hjelpefunksjoner fra de uthevede områdene for å redusere den klyngede kompleksiteten.
    .label = { $lines ->
        [one] Kompleksitetstopp { $index } strekker seg over { $lines } linje.
       *[other] Kompleksitetstopp { $index } strekker seg over { $lines } linjer.
    }
