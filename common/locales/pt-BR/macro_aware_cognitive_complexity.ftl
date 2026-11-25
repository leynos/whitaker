## A complexidade cognitiva do código escrito à mão deve permanecer dentro do limite configurado.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` tem complexidade cognitiva de { $score } (limite: { $limit }).
    .note = As ramificações geradas pela expansão de macros não são contabilizadas.
    .help = Divida a função em funções menores com nomes claros.
