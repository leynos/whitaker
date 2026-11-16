## Aufrufe von `expect` außerhalb von Testkontexten einschränken.

no_expect_outside_tests = Vermeiden Sie Aufrufe von `expect` auf { $receiver } außerhalb von reinem Testcode.
    .note = Der Aufruf stammt aus „{ $context }“; dies wird nicht als Test erkannt.
    .help = { $handling ->
        [option] Behandeln Sie die Variante `None` von { $receiver } oder verschieben Sie den Code in einen Test.
        [result] Behandeln Sie die Variante `Err` von { $receiver } oder verschieben Sie den Code in einen Test.
       *[other] Behandeln Sie den Fehlerpfad für { $receiver } oder verschieben Sie den Code in einen Test.
    }
