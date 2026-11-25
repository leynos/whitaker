## Os testes devem evitar seções de exemplo na documentação.

test_must_not_have_example = Remova as seções de exemplo da documentação do teste { $test }.
    .note = A documentação de { $test } contém o seguinte elemento: “{ $reason }”.
    .help = Remova o exemplo ou mova-o para uma documentação independente voltada aos usuários.
