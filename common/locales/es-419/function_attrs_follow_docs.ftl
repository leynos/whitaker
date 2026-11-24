## Diagnósticos del lint de orden de los atributos de función.

function_attrs_follow_docs = Los comentarios de documentación de “{ $subject }” deben preceder a los demás atributos externos.
    .note = El atributo externo { $attribute } aparece antes del comentario de documentación.
    .help = Mueva el comentario de documentación para que aparezca antes de { $attribute } en el elemento.
