## Begränsa anrop till `expect` utanför testsammanhang.

no_expect_outside_tests = Undvik att anropa `expect` på { $receiver } utanför kod som bara används i tester.
    .note = Anropet kommer från ”{ $context }”, som inte känns igen som ett test.
    .help = { $handling ->
        [option] Hantera varianten `None` för { $receiver } eller flytta koden till ett test.
        [result] Hantera varianten `Err` för { $receiver } eller flytta koden till ett test.
       *[other] Hantera felvägen för { $receiver } eller flytta koden till ett test.
    }
