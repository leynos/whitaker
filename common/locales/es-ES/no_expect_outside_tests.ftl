## Restringir las llamadas a `expect` fuera de los contextos de prueba.

no_expect_outside_tests = Evita llamar a `expect` sobre { $receiver } fuera del código exclusivo de pruebas.
    .note = La llamada se origina en «{ $context }», que no se reconoce como una prueba.
    .help = { $handling ->
        [option] Gestiona la variante `None` de { $receiver } o mueve el código a una prueba.
        [result] Gestiona la variante `Err` de { $receiver } o mueve el código a una prueba.
       *[other] Gestiona la ruta de error de { $receiver } o mueve el código a una prueba.
    }
