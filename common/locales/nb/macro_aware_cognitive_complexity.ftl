## Den kognitive kompleksiteten i skrevet kode bør holde seg innenfor den konfigurerte grensen.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` har en kognitiv kompleksitet på { $score } (grense { $limit }).
    .note = Forgrening som er generert av makroekspansjon, telles ikke.
    .help = Del funksjonen opp i mindre funksjoner med tydelige navn.
