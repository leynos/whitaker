## A complexidade cognitiva do código escrito à mão deve manter-se dentro do limite configurado.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` tem uma complexidade cognitiva de { $score } (limite: { $limit }).
    .note = A ramificação gerada pela expansão de macros não é contabilizada.
    .help = Dividir a função em funções mais pequenas com nomes claros.
