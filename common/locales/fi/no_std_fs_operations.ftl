## Rajoita `std::fs`-operaatioita kyvykkyyspohjaisen I/O:n varmistamiseksi.

no_std_fs_operations = std::fs-operaatio `{ $operation }` ohittaa kyvykkyyspohjaisen tiedostojärjestelmäkäytännön.
    .note = std::fs käyttää ympäristön työhakemistoa; ota sen sijaan vastaan `cap_std::fs::Dir`-kahvoja ja camino-polkuja, jotta kutsujat valitsevat kyvykkyyspinnan.
    .help = Välitä `cap_std::fs::Dir` sekä tyyppien `camino::Utf8Path`/`Utf8PathBuf` parametrit rajapintojesi läpi sen sijaan, että kutsuisit std::fs-moduulia suoraan.
