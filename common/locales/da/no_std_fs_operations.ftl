## Begræns `std::fs`-operationer for at håndhæve kapabilitetsbaseret I/O.

no_std_fs_operations = std::fs-operationen `{ $operation }` omgår den kapabilitetsbaserede filsystempolitik.
    .note = std::fs rører ved den omgivende arbejdsmappe; modtag i stedet `cap_std::fs::Dir`-handles og camino-stier, så kalderne vælger kapabilitetsfladen.
    .help = Send `cap_std::fs::Dir` samt parametre af typen `camino::Utf8Path`/`Utf8PathBuf` gennem dine API'er i stedet for at kalde std::fs direkte.
