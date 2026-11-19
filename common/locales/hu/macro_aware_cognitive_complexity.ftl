## A kézzel írt kód kognitív komplexitásának a beállított korláton belül kell maradnia.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` kognitív komplexitása { $score } (korlát: { $limit }).
    .note = A makrókifejtés által létrehozott elágazások nem számítanak bele.
    .help = Bontsa a függvényt kisebb, egyértelmű nevű függvényekre.
