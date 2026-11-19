## Detector de complejidad “Bumpy Road”.

bumpy_road_function = Hay varios grupos de lógica condicional anidada en `{ $name }`.
    .note = { $count ->
        [one] Se detectó { $count } bache de complejidad por encima del umbral { $threshold }.
       *[other] Se detectaron { $count } baches de complejidad por encima del umbral { $threshold }.
    }
    .help = Extraiga funciones auxiliares de las regiones resaltadas para reducir la complejidad agrupada.
    .label = { $lines ->
        [one] El bache de complejidad { $index } abarca { $lines } línea.
       *[other] El bache de complejidad { $index } abarca { $lines } líneas.
    }
