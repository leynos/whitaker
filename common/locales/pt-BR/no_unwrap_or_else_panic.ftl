## Desencorajar fallbacks de unwrap_or_else que entram em pânico.

no_unwrap_or_else_panic = Substitua `unwrap_or_else` em { $receiver } por um fallback que não entre em pânico.
    .note = A closure fornecida a `unwrap_or_else` provoca um pânico.
    .help = Propague o erro ou use `expect` com uma mensagem descritiva em vez de entrar em pânico.
