## Restringir chamadas a `expect` fora de contextos de teste.

no_expect_outside_tests = Evite chamar `expect` em { $receiver } fora de código exclusivo de testes.
    .note = A chamada tem origem em “{ $context }”, que não é reconhecido como um teste.
    .help = { $handling ->
        [option] Trate a variante `None` de { $receiver } ou mova o código para um teste.
        [result] Trate a variante `Err` de { $receiver } ou mova o código para um teste.
       *[other] Trate o caminho de erro de { $receiver } ou mova o código para um teste.
    }
