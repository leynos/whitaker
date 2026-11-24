## Limitare le operazioni std::fs per imporre un I/O basato sulle capability.

no_std_fs_operations = L’operazione std::fs `{ $operation }` aggira la politica di accesso al file system basata sulle capability.
    .note = std::fs agisce sulla directory di lavoro ambientale; accettare invece handle `cap_std::fs::Dir` e percorsi `camino`, in modo che siano i chiamanti a scegliere la superficie di capability.
    .help = Passare parametri `cap_std::fs::Dir` insieme a `camino::Utf8Path`/`Utf8PathBuf` attraverso le proprie API invece di chiamare direttamente std::fs.
