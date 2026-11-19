## Desencorajar alternativas de contingência de unwrap_or_else que entram em pânico.

no_unwrap_or_else_panic = Substituir `unwrap_or_else` em { $receiver } por uma alternativa de contingência que não entre em pânico.
    .note = A closure fornecida a `unwrap_or_else` provoca um pânico.
    .help = Propagar o erro ou utilizar `expect` com uma mensagem descritiva em vez de entrar em pânico.
