## Az std::fs műveletek korlátozása a képességalapú I/O kikényszerítéséhez.

no_std_fs_operations = A(z) `{ $operation }` std::fs művelet megkerüli a képességalapú fájlrendszer-szabályzatot.
    .note = Az std::fs a környezeti munkakönyvtárat használja; fogadjon inkább `cap_std::fs::Dir` leírókat és camino elérési utakat, hogy a hívók választhassák meg a képességek körét.
    .help = Adjon át `cap_std::fs::Dir`, valamint `camino::Utf8Path`/`Utf8PathBuf` paramétereket az API-kon keresztül az std::fs közvetlen hívása helyett.

no_std_fs_operations_unknown_keys = Ismeretlen { $keys } kulcsok a(z) [no_std_fs_operations] szakaszban a(z) no_std_fs_operations { $version } számára (revision: { $revision }) figyelmen kívül maradnak.
    .note = A szakasz többi kulcsa továbbra is érvényes; az ismeretlen kulcsoknak nincs hatásuk.
    .help = Frissítse a Whitaker csomagot egy olyan revisionre, amely ismeri ezeket a kulcsokat, vagy távolítsa el őket a dylint.toml fájlból.

no_std_fs_operations_discarded_section = A(z) no_std_fs_operations { $version } (revision: { $revision }) [no_std_fs_operations] konfigurációja el lesz dobva: { $error }
    .note = Nem érvényesül kizárás; a lint az alapértelmezett beállításait használja.
    .help = Javítsa az érvénytelen értéket a dylint.toml fájlban, vagy frissítse a Whitaker csomagot, hogy megértse a kulcsot.
