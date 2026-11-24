## Erkennung von „Bumpy Road“-Komplexität.

bumpy_road_function = Mehrere Cluster verschachtelter Bedingungslogik in `{ $name }`.
    .note = { $count ->
        [one] { $count } Komplexitätsspitze oberhalb des Schwellenwerts { $threshold } erkannt.
       *[other] { $count } Komplexitätsspitzen oberhalb des Schwellenwerts { $threshold } erkannt.
    }
    .help = Lagern Sie die hervorgehobenen Bereiche in Hilfsfunktionen aus, um die gebündelte Komplexität zu verringern.
    .label = { $lines ->
        [one] Komplexitätsspitze { $index } erstreckt sich über { $lines } Zeile.
       *[other] Komplexitätsspitze { $index } erstreckt sich über { $lines } Zeilen.
    }
