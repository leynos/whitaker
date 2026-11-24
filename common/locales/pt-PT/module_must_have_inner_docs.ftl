## Os módulos exigem um comentário de documentação interno inicial.

module_must_have_inner_docs = O módulo { $module } deve começar com um comentário de documentação interno.
    .note = O primeiro item do módulo não é um comentário ao estilo `//!`.
    .help = Explicar o propósito de { $module } através de um comentário de documentação interno no início.
