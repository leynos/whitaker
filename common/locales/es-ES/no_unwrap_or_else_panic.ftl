## Desaconsejar las alternativas de respaldo de unwrap_or_else que entran en pánico.

no_unwrap_or_else_panic = Sustituye `unwrap_or_else` sobre { $receiver } por una alternativa de respaldo que no entre en pánico.
    .note = La clausura proporcionada a `unwrap_or_else` provoca un pánico.
    .help = Propaga el error o usa `expect` con un mensaje descriptivo en lugar de entrar en pánico.
