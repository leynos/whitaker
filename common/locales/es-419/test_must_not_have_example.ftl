## Las pruebas deben evitar las secciones de ejemplo en la documentación.

test_must_not_have_example = Elimine las secciones de ejemplo de la documentación de la prueba { $test }.
    .note = La documentación de { $test } contiene el siguiente elemento: “{ $reason }”.
    .help = Quite el ejemplo o muévalo a una documentación independiente destinada a los usuarios.
