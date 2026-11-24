## Complexitatea cognitivă a codului scris de mână trebuie să rămână în limita configurată.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` are o complexitate cognitivă de { $score } (limită: { $limit }).
    .note = Ramificarea generată de expandarea macrourilor nu este luată în calcul.
    .help = Împărțiți funcția în funcții mai mici, cu nume clare.
