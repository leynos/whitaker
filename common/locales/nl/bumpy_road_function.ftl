## Detector voor “Bumpy Road”-complexiteit.

bumpy_road_function = Meerdere clusters van geneste voorwaardelijke logica in `{ $name }`.
    .note = { $count ->
        [one] { $count } complexiteitspiek gedetecteerd boven de drempelwaarde { $threshold }.
       *[other] { $count } complexiteitspieken gedetecteerd boven de drempelwaarde { $threshold }.
    }
    .help = Verplaats de gemarkeerde gebieden naar hulpfuncties om de geclusterde complexiteit te verminderen.
    .label = { $lines ->
        [one] Complexiteitspiek { $index } beslaat { $lines } regel.
       *[other] Complexiteitspiek { $index } beslaat { $lines } regels.
    }
