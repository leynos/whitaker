## Restreindre les appels à `expect` hors des contextes de test.

no_expect_outside_tests = Évitez d’appeler `expect` sur { $receiver } en dehors du code réservé aux tests.
    .note = L’appel provient de « { $context } », qui n’est pas reconnu comme un test.
    .help = { $handling ->
        [option] Traitez la variante `None` de { $receiver } ou déplacez le code dans un test.
        [result] Traitez la variante `Err` de { $receiver } ou déplacez le code dans un test.
       *[other] Traitez le chemin d’erreur de { $receiver } ou déplacez le code dans un test.
    }
