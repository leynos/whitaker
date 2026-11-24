## La complessità cognitiva del codice scritto a mano deve restare entro il limite configurato.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` ha una complessità cognitiva di { $score } (limite: { $limit }).
    .note = Le diramazioni generate dall’espansione delle macro non vengono conteggiate.
    .help = Suddividere la funzione in funzioni più piccole con nomi chiari.
