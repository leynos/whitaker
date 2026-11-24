## Kognitivní složitost ručně psaného kódu má zůstat v nastaveném limitu.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` má kognitivní složitost { $score } (limit { $limit }).
    .note = Větvení vygenerované expanzí maker se nezapočítává.
    .help = Rozdělte funkci na menší funkce s výstižnými názvy.
