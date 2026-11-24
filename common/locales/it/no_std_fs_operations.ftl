## Limitare le operazioni std::fs per imporre un I/O basato sulle capability.

no_std_fs_operations = L’operazione std::fs `{ $operation }` aggira la politica di accesso al file system basata sulle capability.
    .note = std::fs agisce sulla directory di lavoro ambientale; accettare invece handle `cap_std::fs::Dir` e percorsi `camino`, in modo che siano i chiamanti a scegliere la superficie di capability.
    .help = Passare parametri `cap_std::fs::Dir` insieme a `camino::Utf8Path`/`Utf8PathBuf` attraverso le proprie API invece di chiamare direttamente std::fs.

no_std_fs_operations_unknown_keys = Le chiavi sconosciute { $keys } in [no_std_fs_operations] per no_std_fs_operations { $version } (revisione { $revision }) vengono ignorate.
    .note = Le altre chiavi di questa sezione restano valide; le chiavi non riconosciute non hanno effetto.
    .help = Aggiornare la suite Whitaker a una revisione che comprenda queste chiavi, oppure rimuoverle da dylint.toml.

no_std_fs_operations_discarded_section = La configurazione [no_std_fs_operations] per no_std_fs_operations { $version } (revisione { $revision }) viene scartata: { $error }
    .note = Non si applica alcuna esclusione; il lint usa le impostazioni predefinite.
    .help = Correggere il valore non valido in dylint.toml, oppure aggiornare la suite Whitaker perché comprenda la chiave.
