## Beperk `std::fs`-bewerkingen om capability-gebaseerde I/O af te dwingen.

no_std_fs_operations = De std::fs-bewerking `{ $operation }` omzeilt het capability-gebaseerde bestandssysteembeleid.
    .note = std::fs raakt de omgevingswerkmap; accepteer in plaats daarvan `cap_std::fs::Dir`-handles en camino-paden, zodat aanroepers het capability-oppervlak kiezen.
    .help = Geef `cap_std::fs::Dir` plus parameters van het type `camino::Utf8Path`/`Utf8PathBuf` door via je API's in plaats van std::fs rechtstreeks aan te roepen.

no_std_fs_operations_unknown_keys = Onbekende sleutels { $keys } in [no_std_fs_operations] voor no_std_fs_operations { $version } (revisie { $revision }) worden genegeerd.
    .note = De overige sleutels in deze sectie blijven gelden; de onbekende sleutels hebben geen effect.
    .help = Werk de Whitaker-suite bij naar een revisie die deze sleutels kent, of verwijder ze uit dylint.toml.

no_std_fs_operations_discarded_section = De configuratie [no_std_fs_operations] voor no_std_fs_operations { $version } (revisie { $revision }) wordt verworpen: { $error }
    .note = Er gelden geen uitsluitingen; de lint gebruikt de standaardinstellingen.
    .help = Corrigeer de ongeldige waarde in dylint.toml, of werk de Whitaker-suite bij zodat deze de sleutel begrijpt.
