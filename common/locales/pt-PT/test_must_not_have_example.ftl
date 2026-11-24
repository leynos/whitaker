## Os testes devem evitar secções de exemplo na documentação.

test_must_not_have_example = Remover as secções de exemplo da documentação do teste { $test }.
    .note = A documentação de { $test } contém o seguinte elemento: «{ $reason }».
    .help = Eliminar o exemplo ou movê-lo para documentação independente destinada aos utilizadores.
