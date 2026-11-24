## Restringir as chamadas a `expect` fora de contextos de teste.

no_expect_outside_tests = Evitar chamar `expect` em { $receiver } fora de código exclusivo de testes.
    .note = A chamada tem origem em «{ $context }», que não é reconhecido como um teste.
    .help = { $handling ->
        [option] Tratar a variante `None` de { $receiver } ou mover o código para um teste.
        [result] Tratar a variante `Err` de { $receiver } ou mover o código para um teste.
       *[other] Tratar o caminho de erro de { $receiver } ou mover o código para um teste.
    }
