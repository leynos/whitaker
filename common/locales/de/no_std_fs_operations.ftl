## `std::fs`-Operationen einschränken, um Capability-basierte Ein-/Ausgabe durchzusetzen.

no_std_fs_operations = Die std::fs-Operation `{ $operation }` umgeht die Capability-basierte Dateisystemrichtlinie.
    .note = std::fs greift auf das umgebende Arbeitsverzeichnis zu; nehmen Sie stattdessen `cap_std::fs::Dir`-Handles und camino-Pfade entgegen, damit die Aufrufer die Capability-Oberfläche wählen.
    .help = Reichen Sie `cap_std::fs::Dir` sowie Parameter vom Typ `camino::Utf8Path`/`Utf8PathBuf` durch Ihre APIs, statt std::fs direkt aufzurufen.
