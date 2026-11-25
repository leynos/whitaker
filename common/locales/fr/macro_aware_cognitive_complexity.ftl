## La complexité cognitive du code écrit à la main doit rester dans la limite configurée.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` a une complexité cognitive de { $score } (limite : { $limit }).
    .note = Les embranchements générés par l’expansion de macros ne sont pas comptés.
    .help = Scindez la fonction en fonctions plus petites aux noms explicites.
