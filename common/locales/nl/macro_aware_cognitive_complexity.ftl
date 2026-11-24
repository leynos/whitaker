## De cognitieve complexiteit van geschreven code moet binnen de geconfigureerde limiet blijven.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` heeft een cognitieve complexiteit van { $score } (limiet { $limit }).
    .note = Vertakkingen die door macro-expansie zijn gegenereerd, worden niet meegeteld.
    .help = Splits de functie op in kleinere functies met duidelijke namen.
