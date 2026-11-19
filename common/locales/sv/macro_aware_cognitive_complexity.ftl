## Den kognitiva komplexiteten i skriven kod bör hålla sig inom den konfigurerade gränsen.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` har en kognitiv komplexitet på { $score } (gräns { $limit }).
    .note = Förgrening som genereras av makroexpansion räknas inte.
    .help = Dela upp funktionen i mindre funktioner med tydliga namn.
