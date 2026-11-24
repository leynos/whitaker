## Restringir as operações de std::fs para impor E/S baseada em capacidades.

no_std_fs_operations = A operação de std::fs `{ $operation }` contorna a política de sistema de ficheiros baseada em capacidades.
    .note = std::fs atua sobre o diretório de trabalho ambiente; aceitar antes handles `cap_std::fs::Dir` e caminhos do `camino`, para que sejam os chamadores a escolher a superfície de capacidades.
    .help = Passar parâmetros `cap_std::fs::Dir` juntamente com `camino::Utf8Path`/`Utf8PathBuf` através das APIs em vez de chamar std::fs diretamente.
