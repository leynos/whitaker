## A pánikot kiváltó unwrap_or_else tartalék megoldások kerülése.

no_unwrap_or_else_panic = Cserélje a(z) { $receiver } értéken hívott unwrap_or_else hívást pánikmentes tartalék megoldásra.
    .note = Az unwrap_or_else számára átadott lezárás pánikot vált ki.
    .help = Adja tovább a hibát, vagy pánik kiváltása helyett használjon `expect` hívást leíró üzenettel.
