## Detetor de complexidade «Bumpy Road».

bumpy_road_function = Existem vários agrupamentos de lógica condicional aninhada em `{ $name }`.
    .note = { $count ->
        [one] Foi detetada { $count } lomba de complexidade acima do limiar { $threshold }.
       *[other] Foram detetadas { $count } lombas de complexidade acima do limiar { $threshold }.
    }
    .help = Extrair funções auxiliares das regiões realçadas para reduzir a complexidade agrupada.
    .label = { $lines ->
        [one] A lomba de complexidade { $index } abrange { $lines } linha.
       *[other] A lomba de complexidade { $index } abrange { $lines } linhas.
    }
