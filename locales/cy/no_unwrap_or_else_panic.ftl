## Peidiwch â dibynnu ar `unwrap_or_else(|| panic!(...))`.

no_unwrap_or_else_panic = Newidiwch `unwrap_or_else(|| panic!(..))` ar { $receiver } i drin gwall yn briodol.
    .note = Mae’r caead yn pasio panic i `unwrap_or_else`.
    .help = Lledaenwch y gwall neu defnyddiwch `expect` gyda neges eglur yn lle hynny.
