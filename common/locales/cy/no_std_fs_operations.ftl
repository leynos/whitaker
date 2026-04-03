## Gwaherddir gweithrediadau std::fs i orfodi I/O sy’n seiliedig ar gapasiti.

no_std_fs_operations = Mae gweithred std::fs `{ $operation }` yn osgoi’r polisi capasiti ar gyfer y system ffeiliau.
    .note = Mae std::fs yn cyffwrdd â’r cyfeiriadur amgylcheddol; derbyniwch ddolenni `cap_std::fs::Dir` a llwybrau camino er mwyn i’r galwr ddewis y gallu.
    .help = Pasio `cap_std::fs::Dir` a pharamedrau `camino::Utf8Path`/`Utf8PathBuf` drwy’ch APIau yn hytrach na galw std::fs yn uniongyrchol.
