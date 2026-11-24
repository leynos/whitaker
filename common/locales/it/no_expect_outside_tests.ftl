## Limitare le chiamate a `expect` al di fuori dei contesti di test.

no_expect_outside_tests = Evitare di chiamare `expect` su { $receiver } al di fuori del codice riservato ai test.
    .note = La chiamata ha origine in «{ $context }», che non è riconosciuto come test.
    .help = { $handling ->
        [option] Gestire la variante `None` di { $receiver } oppure spostare il codice in un test.
        [result] Gestire la variante `Err` di { $receiver } oppure spostare il codice in un test.
       *[other] Gestire il percorso di errore di { $receiver } oppure spostare il codice in un test.
    }
