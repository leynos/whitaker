## Bacar std::fs gus cumail ri modail comasan don t-siostam fhaidhlichean.

no_std_fs_operations = Tha gnìomh std::fs `{ $operation }` a’ seachnadh a’ phoileasaidh comasan airson an t-siostaim fhaidhlichean.
    .note = Bidh std::fs a’ dol an sàs leis an eòlas àrainneachd; gabhaibh làmhan `cap_std::fs::Dir` agus slighean camino gus am bi an neach-gairm a’ taghadh na comasan.
    .help = Cuir seachad `cap_std::fs::Dir` agus paramadairean `camino::Utf8Path`/`Utf8PathBuf` tron API seach std::fs a ghairm gu dìreach.

no_std_fs_operations_unknown_keys = A’ seachnadh iuchraichean neo-aithnichte { $keys } ann an [no_std_fs_operations] airson no_std_fs_operations { $version } (ath-sgrùdadh { $revision }).
    .note = Tha iuchraichean eile san roinn seo fhathast a’ buntainn; chan eil buaidh aig na h-iuchraichean nach aithnichear.
    .help = Ùraich an t-sreath Whitaker no thoir air falbh na h-iuchraichean nach aithnichear bho dylint.toml.

no_std_fs_operations_discarded_section = A’ tilgeil air falbh rèiteachadh [no_std_fs_operations] airson no_std_fs_operations { $version } (ath-sgrùdadh { $revision }): { $error }
    .note = Chan eil eisgeachdan sam bith a’ buntainn; tha an lint a’ cleachdadh nan roghainnean bunaiteach.
    .help = Ceartaich an luach mì-dhligheach ann an dylint.toml, no ùraich an t-sreath Whitaker gus am bi e a’ tuigsinn na h-iuchrach.
