## Beperk `std::fs`-bewerkingen om capability-gebaseerde I/O af te dwingen.

no_std_fs_operations = De std::fs-bewerking `{ $operation }` omzeilt het capability-gebaseerde bestandssysteembeleid.
    .note = std::fs raakt de omgevingswerkmap; accepteer in plaats daarvan `cap_std::fs::Dir`-handles en camino-paden, zodat aanroepers het capability-oppervlak kiezen.
    .help = Geef `cap_std::fs::Dir` plus parameters van het type `camino::Utf8Path`/`Utf8PathBuf` door via je API's in plaats van std::fs rechtstreeks aan te roepen.
