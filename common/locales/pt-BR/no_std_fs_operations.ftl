## Restringir operações de std::fs para impor E/S baseada em capacidades.

no_std_fs_operations = A operação de std::fs `{ $operation }` contorna a política de sistema de arquivos baseada em capacidades.
    .note = std::fs atua sobre o diretório de trabalho ambiente; aceite handles `cap_std::fs::Dir` e caminhos do `camino` para que os chamadores escolham a superfície de capacidades.
    .help = Passe parâmetros `cap_std::fs::Dir` junto com `camino::Utf8Path`/`Utf8PathBuf` por meio de suas APIs em vez de chamar std::fs diretamente.
