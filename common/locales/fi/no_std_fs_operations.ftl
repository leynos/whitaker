## Rajoita `std::fs`-operaatioita kyvykkyyspohjaisen I/O:n varmistamiseksi.

no_std_fs_operations = std::fs-operaatio `{ $operation }` ohittaa kyvykkyyspohjaisen tiedostojärjestelmäkäytännön.
    .note = std::fs käyttää ympäristön työhakemistoa; ota sen sijaan vastaan `cap_std::fs::Dir`-kahvoja ja camino-polkuja, jotta kutsujat valitsevat kyvykkyyspinnan.
    .help = Välitä `cap_std::fs::Dir` sekä tyyppien `camino::Utf8Path`/`Utf8PathBuf` parametrit rajapintojesi läpi sen sijaan, että kutsuisit std::fs-moduulia suoraan.

no_std_fs_operations_unknown_keys = Ohitetaan tuntemattomat avaimet { $keys } kohdassa [no_std_fs_operations] tuotteelle no_std_fs_operations { $version } (versio { $revision }).
    .note = Tämän osion muut avaimet ovat edelleen voimassa; tuntemattomilla avaimilla ei ole vaikutusta.
    .help = Päivitä Whitaker-sarja versioon, joka tuntee nämä avaimet, tai poista ne tiedostosta dylint.toml.

no_std_fs_operations_discarded_section = Hylätään [no_std_fs_operations]-asetukset tuotteelle no_std_fs_operations { $version } (versio { $revision }): { $error }
    .note = Poikkeuksia ei sovelleta; lint käyttää oletusasetuksiaan.
    .help = Korjaa virheellinen arvo tiedostossa dylint.toml tai päivitä Whitaker-sarja ymmärtämään avain.
