## Beperk aanroepen van `expect` buiten testcontexten.

no_expect_outside_tests = Vermijd het aanroepen van `expect` op { $receiver } buiten code die alleen voor tests is bedoeld.
    .note = De aanroep komt uit “{ $context }”, en dat wordt niet als test herkend.
    .help = { $handling ->
        [option] Handel de variant `None` van { $receiver } af of verplaats de code naar een test.
        [result] Handel de variant `Err` van { $receiver } af of verplaats de code naar een test.
       *[other] Handel het foutpad voor { $receiver } af of verplaats de code naar een test.
    }
