## Restringir las llamadas a `expect` fuera de los contextos de prueba.

no_expect_outside_tests = Evite llamar a `expect` sobre { $receiver } fuera del código exclusivo de pruebas.
    .note = La llamada se origina en “{ $context }”, que no se reconoce como una prueba.
    .help = { $handling ->
        [option] Maneje la variante `None` de { $receiver } o mueva el código a una prueba.
        [result] Maneje la variante `Err` de { $receiver } o mueva el código a una prueba.
       *[other] Maneje la ruta de error de { $receiver } o mueva el código a una prueba.
    }
