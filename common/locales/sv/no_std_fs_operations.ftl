## Begränsa `std::fs`-operationer för att upprätthålla kapabilitetsbaserad I/O.

no_std_fs_operations = std::fs-operationen `{ $operation }` kringgår den kapabilitetsbaserade filsystemspolicyn.
    .note = std::fs rör den omgivande arbetskatalogen; ta i stället emot `cap_std::fs::Dir`-handtag och camino-sökvägar så att anroparna väljer kapabilitetsytan.
    .help = Skicka `cap_std::fs::Dir` samt parametrar av typen `camino::Utf8Path`/`Utf8PathBuf` genom dina API:er i stället för att anropa std::fs direkt.
