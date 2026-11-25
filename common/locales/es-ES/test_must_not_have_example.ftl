## Las pruebas deben evitar las secciones de ejemplo en la documentación.

test_must_not_have_example = Elimina las secciones de ejemplo de la documentación de la prueba { $test }.
    .note = La documentación de { $test } contiene el siguiente elemento: «{ $reason }».
    .help = Quita el ejemplo o muévelo a una documentación independiente destinada a los usuarios.
