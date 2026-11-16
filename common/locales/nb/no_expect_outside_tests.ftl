## Begrens kall til `expect` utenfor testkontekster.

no_expect_outside_tests = Unngå å kalle `expect` på { $receiver } utenfor kode som bare brukes i tester.
    .note = Kallet kommer fra «{ $context }», som ikke gjenkjennes som en test.
    .help = { $handling ->
        [option] Håndter varianten `None` for { $receiver }, eller flytt koden inn i en test.
        [result] Håndter varianten `Err` for { $receiver }, eller flytt koden inn i en test.
       *[other] Håndter feilstien for { $receiver }, eller flytt koden inn i en test.
    }
