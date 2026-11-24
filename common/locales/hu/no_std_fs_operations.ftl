## Az std::fs műveletek korlátozása a képességalapú I/O kikényszerítéséhez.

no_std_fs_operations = A(z) `{ $operation }` std::fs művelet megkerüli a képességalapú fájlrendszer-szabályzatot.
    .note = Az std::fs a környezeti munkakönyvtárat használja; fogadjon inkább `cap_std::fs::Dir` leírókat és camino elérési utakat, hogy a hívók választhassák meg a képességek körét.
    .help = Adjon át `cap_std::fs::Dir`, valamint `camino::Utf8Path`/`Utf8PathBuf` paramétereket az API-kon keresztül az std::fs közvetlen hívása helyett.
