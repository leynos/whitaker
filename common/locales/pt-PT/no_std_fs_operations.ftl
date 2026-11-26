## Restringir as operações de std::fs para impor E/S baseada em capacidades.

no_std_fs_operations = A operação de std::fs `{ $operation }` contorna a política de sistema de ficheiros baseada em capacidades.
    .note = std::fs atua sobre o diretório de trabalho ambiente; aceitar antes handles `cap_std::fs::Dir` e caminhos do `camino`, para que sejam os chamadores a escolher a superfície de capacidades.
    .help = Passar parâmetros `cap_std::fs::Dir` juntamente com `camino::Utf8Path`/`Utf8PathBuf` através das APIs em vez de chamar std::fs diretamente.

no_std_fs_operations_unknown_keys = A ignorar chaves desconhecidas { $keys } em [no_std_fs_operations] para no_std_fs_operations { $version } (revisão { $revision }).
    .note = As restantes chaves desta secção continuam a aplicar-se; as chaves não reconhecidas não têm efeito.
    .help = Atualize o conjunto Whitaker para uma revisão que compreenda estas chaves, ou remova-as de dylint.toml.

no_std_fs_operations_discarded_section = A descartar a configuração [no_std_fs_operations] para no_std_fs_operations { $version } (revisão { $revision }): { $error }
    .note = Não se aplica qualquer exclusão; o lint está a usar as definições predefinidas.
    .help = Corrija o valor inválido em dylint.toml, ou atualize o conjunto Whitaker para que compreenda a chave.
