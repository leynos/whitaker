## Restricționarea operațiilor std::fs pentru a impune I/O bazat pe capabilități.

no_std_fs_operations = Operația std::fs `{ $operation }` ocolește politica pentru sistemul de fișiere bazată pe capabilități.
    .note = std::fs acționează asupra directorului de lucru ambiant; acceptați în schimb handle-uri `cap_std::fs::Dir` și căi `camino`, astfel încât apelanții să aleagă suprafața de capabilități.
    .help = Transmiteți parametri `cap_std::fs::Dir` împreună cu `camino::Utf8Path`/`Utf8PathBuf` prin API-urile dumneavoastră, în loc să apelați direct std::fs.
