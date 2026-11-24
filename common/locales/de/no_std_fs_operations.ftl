## `std::fs`-Operationen einschränken, um Capability-basierte Ein-/Ausgabe durchzusetzen.

no_std_fs_operations = Die std::fs-Operation `{ $operation }` umgeht die Capability-basierte Dateisystemrichtlinie.
    .note = std::fs greift auf das umgebende Arbeitsverzeichnis zu; nehmen Sie stattdessen `cap_std::fs::Dir`-Handles und camino-Pfade entgegen, damit die Aufrufer die Capability-Oberfläche wählen.
    .help = Reichen Sie `cap_std::fs::Dir` sowie Parameter vom Typ `camino::Utf8Path`/`Utf8PathBuf` durch Ihre APIs, statt std::fs direkt aufzurufen.

no_std_fs_operations_unknown_keys = Unbekannte Schlüssel { $keys } in [no_std_fs_operations] für no_std_fs_operations { $version } (Revision { $revision }) werden ignoriert.
    .note = Die übrigen Schlüssel in diesem Abschnitt gelten weiterhin; die unbekannten Schlüssel haben keine Wirkung.
    .help = Aktualisieren Sie die Whitaker-Suite auf eine Revision, die diese Schlüssel kennt, oder entfernen Sie sie aus dylint.toml.

no_std_fs_operations_discarded_section = Die Konfiguration [no_std_fs_operations] für no_std_fs_operations { $version } (Revision { $revision }) wird verworfen: { $error }
    .note = Es gelten keine Ausnahmen; der Lint verwendet seine Standardeinstellungen.
    .help = Korrigieren Sie den ungültigen Wert in dylint.toml, oder aktualisieren Sie die Whitaker-Suite, damit sie den Schlüssel versteht.
