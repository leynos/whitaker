## Omezení volání expect mimo testovací kontexty.

no_expect_outside_tests = Nevolejte `expect` na hodnotě { $receiver } mimo kód určený pouze pro testy.
    .note = Volání pochází z kontextu „{ $context }“, který není rozpoznán jako test.
    .help = { $handling ->
        [option] Ošetřete variantu `None` hodnoty { $receiver } nebo kód přesuňte do testu.
        [result] Ošetřete variantu `Err` hodnoty { $receiver } nebo kód přesuňte do testu.
       *[other] Ošetřete chybovou cestu hodnoty { $receiver } nebo kód přesuňte do testu.
    }
