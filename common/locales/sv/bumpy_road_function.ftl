## Detektor för ”Bumpy Road”-komplexitet.

bumpy_road_function = Flera kluster av nästlad villkorslogik i `{ $name }`.
    .note = { $count ->
        [one] Hittade { $count } komplexitetstopp över tröskelvärdet { $threshold }.
       *[other] Hittade { $count } komplexitetstoppar över tröskelvärdet { $threshold }.
    }
    .help = Bryt ut hjälpfunktioner från de markerade områdena för att minska den klustrade komplexiteten.
    .label = { $lines ->
        [one] Komplexitetstopp { $index } sträcker sig över { $lines } rad.
       *[other] Komplexitetstopp { $index } sträcker sig över { $lines } rader.
    }
