## Detektor for “Bumpy Road”-kompleksitet.

bumpy_road_function = Flere klynger af indlejret betinget logik i `{ $name }`.
    .note = { $count ->
        [one] Fandt { $count } kompleksitetstop over tærskelværdien { $threshold }.
       *[other] Fandt { $count } kompleksitetstoppe over tærskelværdien { $threshold }.
    }
    .help = Udtræk hjælpefunktioner fra de fremhævede områder for at reducere den klyngede kompleksitet.
    .label = { $lines ->
        [one] Kompleksitetstop { $index } strækker sig over { $lines } linje.
       *[other] Kompleksitetstop { $index } strækker sig over { $lines } linjer.
    }
