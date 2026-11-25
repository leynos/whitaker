## Begræns kald af `expect` uden for testkontekster.

no_expect_outside_tests = Undgå at kalde `expect` på { $receiver } uden for kode, der kun bruges til test.
    .note = Kaldet stammer fra “{ $context }”, som ikke genkendes som en test.
    .help = { $handling ->
        [option] Håndter varianten `None` for { $receiver }, eller flyt koden ind i en test.
        [result] Håndter varianten `Err` for { $receiver }, eller flyt koden ind i en test.
       *[other] Håndter fejlstien for { $receiver }, eller flyt koden ind i en test.
    }
