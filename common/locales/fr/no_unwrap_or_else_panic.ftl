## Décourager les solutions de repli unwrap_or_else qui paniquent.

no_unwrap_or_else_panic = Remplacez `unwrap_or_else` sur { $receiver } par une solution de repli qui ne panique pas.
    .note = La fermeture fournie à `unwrap_or_else` déclenche une panique.
    .help = Propagez l’erreur ou utilisez `expect` avec un message descriptif au lieu de paniquer.
