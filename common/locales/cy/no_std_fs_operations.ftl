## Gwaherddir gweithrediadau std::fs i orfodi I/O sy’n seiliedig ar gapasiti.

no_std_fs_operations = Mae gweithred std::fs `{ $operation }` yn osgoi’r polisi capasiti ar gyfer y system ffeiliau.
    .note = Mae std::fs yn cyffwrdd â’r cyfeiriadur amgylcheddol; derbyniwch ddolenni `cap_std::fs::Dir` a llwybrau camino er mwyn i’r galwr ddewis y gallu.
    .help = Pasio `cap_std::fs::Dir` a pharamedrau `camino::Utf8Path`/`Utf8PathBuf` drwy’ch APIau yn hytrach na galw std::fs yn uniongyrchol.

no_std_fs_operations_unknown_keys = Yn anwybyddu allwedd(au) anhysbys { $keys } yn [no_std_fs_operations] ar gyfer no_std_fs_operations { $version } (adolygiad { $revision }).
    .note = Mae allweddi eraill yn yr adran hon yn dal i fod yn berthnasol; nid oes effaith gan yr allwedd(au) nas adnabyddir.
    .help = Diweddarwch y gyfres Whitaker neu dilëwch yr allwedd(au) nas adnabyddir o dylint.toml.

no_std_fs_operations_discarded_section = Yn gwaredu cyfluniad [no_std_fs_operations] ar gyfer no_std_fs_operations { $version } (adolygiad { $revision }): { $error }
    .note = Nid oes unrhyw eithriadau yn berthnasol; mae’r lint yn defnyddio’r gosodiadau rhagosodedig.
    .help = Cywirwch y gwerth annilys yn dylint.toml, neu diweddarwch y gyfres Whitaker fel ei bod yn deall yr allwedd.
