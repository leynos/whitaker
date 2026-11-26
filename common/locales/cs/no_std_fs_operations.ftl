## Omezení operací std::fs kvůli vynucení vstupu a výstupu založeného na schopnostech.

no_std_fs_operations = Operace std::fs `{ $operation }` obchází zásady souborového systému založené na schopnostech.
    .note = Modul std::fs pracuje s okolním pracovním adresářem; přijímejte místo toho popisovače `cap_std::fs::Dir` a cesty camino, aby rozsah schopností volili volající.
    .help = Předávejte svými API parametry `cap_std::fs::Dir` a `camino::Utf8Path`/`Utf8PathBuf` místo přímého volání std::fs.

no_std_fs_operations_unknown_keys = Přeskakuji neznámé klíče { $keys } v [no_std_fs_operations] pro no_std_fs_operations { $version } (revize { $revision }).
    .note = Ostatní klíče v této sekci stále platí; nerozpoznané klíče nemají žádný účinek.
    .help = Aktualizujte sadu Whitaker na revizi, která tyto klíče zná, nebo je z dylint.toml odstraňte.

no_std_fs_operations_discarded_section = Zahazuji konfiguraci [no_std_fs_operations] pro no_std_fs_operations { $version } (revize { $revision }): { $error }
    .note = Neplatí žádné výjimky; lint používá výchozí nastavení.
    .help = Opravte neplatnou hodnotu v dylint.toml nebo aktualizujte sadu Whitaker, aby klíč znala.
