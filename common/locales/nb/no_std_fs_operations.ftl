## Begrens `std::fs`-operasjoner for å håndheve kapabilitetsbasert I/O.

no_std_fs_operations = std::fs-operasjonen `{ $operation }` omgår den kapabilitetsbaserte filsystempolicyen.
    .note = std::fs berører den omgivende arbeidsmappen; ta heller imot `cap_std::fs::Dir`-håndtak og camino-stier, slik at kallerne velger kapabilitetsflaten.
    .help = Send `cap_std::fs::Dir` og parametere av typen `camino::Utf8Path`/`Utf8PathBuf` gjennom API-ene dine i stedet for å kalle std::fs direkte.

no_std_fs_operations_unknown_keys = Ignorerer ukjente nøkler { $keys } i [no_std_fs_operations] for no_std_fs_operations { $version } (revisjon { $revision }).
    .note = De øvrige nøklene i denne delen gjelder fortsatt; de ukjente nøklene har ingen virkning.
    .help = Oppdater Whitaker-suiten til en revisjon som forstår disse nøklene, eller fjern dem fra dylint.toml.

no_std_fs_operations_discarded_section = Forkaster [no_std_fs_operations]-konfigurasjonen for no_std_fs_operations { $version } (revisjon { $revision }): { $error }
    .note = Ingen unntak gjelder; lintet bruker standardinnstillingene.
    .help = Rett den ugyldige verdien i dylint.toml, eller oppdater Whitaker-suiten slik at den forstår nøkkelen.
