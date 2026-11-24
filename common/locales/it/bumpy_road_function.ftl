## Rilevatore di complessità «Bumpy Road».

bumpy_road_function = Più gruppi di logica condizionale annidata in `{ $name }`.
    .note = { $count ->
        [one] Rilevato { $count } picco di complessità sopra la soglia { $threshold }.
       *[other] Rilevati { $count } picchi di complessità sopra la soglia { $threshold }.
    }
    .help = Estrarre funzioni ausiliarie dalle regioni evidenziate per ridurre la complessità concentrata.
    .label = { $lines ->
        [one] Il picco di complessità { $index } si estende su { $lines } riga.
       *[other] Il picco di complessità { $index } si estende su { $lines } righe.
    }
