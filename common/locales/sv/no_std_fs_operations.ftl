## Begränsa `std::fs`-operationer för att upprätthålla kapabilitetsbaserad I/O.

no_std_fs_operations = std::fs-operationen `{ $operation }` kringgår den kapabilitetsbaserade filsystemspolicyn.
    .note = std::fs rör den omgivande arbetskatalogen; ta i stället emot `cap_std::fs::Dir`-handtag och camino-sökvägar så att anroparna väljer kapabilitetsytan.
    .help = Skicka `cap_std::fs::Dir` samt parametrar av typen `camino::Utf8Path`/`Utf8PathBuf` genom dina API:er i stället för att anropa std::fs direkt.

no_std_fs_operations_unknown_keys = Okända nycklar { $keys } i [no_std_fs_operations] för no_std_fs_operations { $version } (revision { $revision }) ignoreras.
    .note = Övriga nycklar i det här avsnittet gäller fortfarande; de okända nycklarna har ingen effekt.
    .help = Uppdatera Whitaker-sviten till en revision som förstår de här nycklarna, eller ta bort dem från dylint.toml.

no_std_fs_operations_discarded_section = [no_std_fs_operations]-konfigurationen för no_std_fs_operations { $version } (revision { $revision }) kastas: { $error }
    .note = Inga undantag tillämpas; lintet använder standardinställningarna.
    .help = Rätta det ogiltiga värdet i dylint.toml, eller uppdatera Whitaker-sviten så att den förstår nyckeln.
