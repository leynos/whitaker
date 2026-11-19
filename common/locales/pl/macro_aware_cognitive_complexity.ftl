## Złożoność poznawcza kodu napisanego ręcznie powinna mieścić się w skonfigurowanym limicie.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` ma złożoność poznawczą { $score } (limit: { $limit }).
    .note = Rozgałęzienia wygenerowane przez rozwinięcie makr nie są liczone.
    .help = Podziel funkcję na mniejsze funkcje o jasnych nazwach.
