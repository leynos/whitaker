## Rajoita `expect`-kutsuja testikontekstien ulkopuolella.

no_expect_outside_tests = Vältä kutsumasta `expect`-metodia kohteelle { $receiver } vain testeille tarkoitetun koodin ulkopuolella.
    .note = Kutsu on peräisin kohteesta ”{ $context }”, jota ei tunnisteta testiksi.
    .help = { $handling ->
        [option] Käsittele kohteen { $receiver } variantti `None` tai siirrä koodi testiin.
        [result] Käsittele kohteen { $receiver } variantti `Err` tai siirrä koodi testiin.
       *[other] Käsittele kohteen { $receiver } virhepolku tai siirrä koodi testiin.
    }
