## Tha `expect` toirmisgte taobh a-muigh deuchainnean.

no_expect_outside_tests = Na gairm `expect` air { $receiver } taobh a-muigh còd deuchainnean.
    .note = Tha an gairm a’ tighinn bho { $context } nach eil air aithneachadh mar dheuchainn.
    .help = { $handling ->
        [option] Dèilig ri cùis `None` aig { $receiver } no gluais an còd gu deuchainn.
        [result] Dèilig ri caochladh `Err` aig { $receiver } no gluais an còd gu deuchainn.
       *[other] Dèilig ris an t-slighe mhearachd aig { $receiver } no gluais an còd gu deuchainn.
    }
