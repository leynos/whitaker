## Omezení operací std::fs kvůli vynucení vstupu a výstupu založeného na schopnostech.

no_std_fs_operations = Operace std::fs `{ $operation }` obchází zásady souborového systému založené na schopnostech.
    .note = Modul std::fs pracuje s okolním pracovním adresářem; přijímejte místo toho popisovače `cap_std::fs::Dir` a cesty camino, aby rozsah schopností volili volající.
    .help = Předávejte svými API parametry `cap_std::fs::Dir` a `camino::Utf8Path`/`Utf8PathBuf` místo přímého volání std::fs.
