## Restringir operações de std::fs para impor E/S baseada em capacidades.

no_std_fs_operations = A operação de std::fs `{ $operation }` contorna a política de sistema de arquivos baseada em capacidades.
    .note = std::fs atua sobre o diretório de trabalho ambiente; aceite handles `cap_std::fs::Dir` e caminhos do `camino` para que os chamadores escolham a superfície de capacidades.
    .help = Passe parâmetros `cap_std::fs::Dir` junto com `camino::Utf8Path`/`Utf8PathBuf` por meio de suas APIs em vez de chamar std::fs diretamente.

no_std_fs_operations_unknown_keys = Ignorando chaves desconhecidas { $keys } em [no_std_fs_operations] para no_std_fs_operations { $version } (revisão { $revision }).
    .note = As demais chaves desta seção continuam valendo; as chaves não reconhecidas não têm efeito.
    .help = Atualize o conjunto Whitaker para uma revisão que entenda estas chaves, ou remova-as de dylint.toml.

no_std_fs_operations_discarded_section = Descartando a configuração [no_std_fs_operations] para no_std_fs_operations { $version } (revisão { $revision }): { $error }
    .note = Nenhuma exclusão se aplica; o lint está usando suas configurações padrão.
    .help = Corrija o valor inválido em dylint.toml, ou atualize o conjunto Whitaker para que entenda a chave.
