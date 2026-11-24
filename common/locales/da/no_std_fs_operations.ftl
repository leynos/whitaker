## Begræns `std::fs`-operationer for at håndhæve kapabilitetsbaseret I/O.

no_std_fs_operations = std::fs-operationen `{ $operation }` omgår den kapabilitetsbaserede filsystempolitik.
    .note = std::fs rører ved den omgivende arbejdsmappe; modtag i stedet `cap_std::fs::Dir`-handles og camino-stier, så kalderne vælger kapabilitetsfladen.
    .help = Send `cap_std::fs::Dir` samt parametre af typen `camino::Utf8Path`/`Utf8PathBuf` gennem dine API'er i stedet for at kalde std::fs direkte.

no_std_fs_operations_unknown_keys = Ignorerer ukendte nøgler { $keys } i [no_std_fs_operations] for no_std_fs_operations { $version } (revision { $revision }).
    .note = De øvrige nøgler i dette afsnit gælder stadig; de ukendte nøgler har ingen virkning.
    .help = Opdater Whitaker-suiten til en revision, der forstår disse nøgler, eller fjern dem fra dylint.toml.

no_std_fs_operations_discarded_section = Kasserer [no_std_fs_operations]-konfigurationen for no_std_fs_operations { $version } (revision { $revision }): { $error }
    .note = Ingen undtagelser gælder; lintet bruger standardindstillingerne.
    .help = Ret den ugyldige værdi i dylint.toml, eller opdater Whitaker-suiten, så den forstår nøglen.
