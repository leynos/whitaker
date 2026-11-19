## Diagnósticos do lint de ordenação dos atributos de função.

function_attrs_follow_docs = Os comentários de documentação em “{ $subject }” devem preceder os demais atributos externos.
    .note = O atributo externo { $attribute } aparece antes do comentário de documentação.
    .help = Mova o comentário de documentação para que ele apareça antes de { $attribute } no item.
