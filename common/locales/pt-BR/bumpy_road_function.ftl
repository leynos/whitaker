## Detector de complexidade “Bumpy Road”.

bumpy_road_function = Há vários agrupamentos de lógica condicional aninhada em `{ $name }`.
    .note = { $count ->
        [one] Foi detectada { $count } lombada de complexidade acima do limiar { $threshold }.
       *[other] Foram detectadas { $count } lombadas de complexidade acima do limiar { $threshold }.
    }
    .help = Extraia funções auxiliares das regiões destacadas para reduzir a complexidade agrupada.
    .label = { $lines ->
        [one] A lombada de complexidade { $index } abrange { $lines } linha.
       *[other] A lombada de complexidade { $index } abrange { $lines } linhas.
    }
