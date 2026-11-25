## La complejidad cognitiva del código escrito a mano debe mantenerse dentro del límite configurado.

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` tiene una complejidad cognitiva de { $score } (límite: { $limit }).
    .note = La ramificación generada por la expansión de macros no se cuenta.
    .help = Divida la función en funciones más pequeñas con nombres claros.
