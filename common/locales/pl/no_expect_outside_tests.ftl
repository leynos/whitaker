## Ograniczenie wywołań expect poza kontekstami testowymi.

no_expect_outside_tests = Unikaj wywoływania `expect` na wartości { $receiver } poza kodem przeznaczonym wyłącznie do testów.
    .note = Wywołanie pochodzi z kontekstu „{ $context }”, który nie jest rozpoznawany jako test.
    .help = { $handling ->
        [option] Obsłuż wariant `None` wartości { $receiver } lub przenieś kod do testu.
        [result] Obsłuż wariant `Err` wartości { $receiver } lub przenieś kod do testu.
       *[other] Obsłuż ścieżkę błędu wartości { $receiver } lub przenieś kod do testu.
    }
