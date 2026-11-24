## Begrens `std::fs`-operasjoner for å håndheve kapabilitetsbasert I/O.

no_std_fs_operations = std::fs-operasjonen `{ $operation }` omgår den kapabilitetsbaserte filsystempolicyen.
    .note = std::fs berører den omgivende arbeidsmappen; ta heller imot `cap_std::fs::Dir`-håndtak og camino-stier, slik at kallerne velger kapabilitetsflaten.
    .help = Send `cap_std::fs::Dir` og parametere av typen `camino::Utf8Path`/`Utf8PathBuf` gjennom API-ene dine i stedet for å kalle std::fs direkte.
