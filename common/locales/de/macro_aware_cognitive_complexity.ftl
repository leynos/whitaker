## Die kognitive Komplexität des geschriebenen Codes soll innerhalb des konfigurierten Grenzwerts bleiben.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` hat eine kognitive Komplexität von { $score } (Grenzwert { $limit }).
    .note = Durch Makroexpansion erzeugte Verzweigungen werden nicht gezählt.
    .help = Teilen Sie die Funktion in kleinere Funktionen mit aussagekräftigen Namen auf.
