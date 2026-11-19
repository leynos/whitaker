## Den kognitive kompleksitet af skrevet kode bør holde sig inden for den konfigurerede grænse.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` har en kognitiv kompleksitet på { $score } (grænse { $limit }).
    .note = Forgrening, der er genereret af makroudvidelse, tælles ikke med.
    .help = Opdel funktionen i mindre funktioner med tydelige navne.
