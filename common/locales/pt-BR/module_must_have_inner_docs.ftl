## Os módulos exigem um comentário de documentação interno inicial.

module_must_have_inner_docs = O módulo { $module } deve começar com um comentário de documentação interno.
    .note = O primeiro item do módulo não é um comentário no estilo `//!`.
    .help = Explique o propósito de { $module } adicionando um comentário de documentação interno no início.
