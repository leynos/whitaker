## Restricționarea operațiilor std::fs pentru a impune I/O bazat pe capabilități.

no_std_fs_operations = Operația std::fs `{ $operation }` ocolește politica pentru sistemul de fișiere bazată pe capabilități.
    .note = std::fs acționează asupra directorului de lucru ambiant; acceptați în schimb handle-uri `cap_std::fs::Dir` și căi `camino`, astfel încât apelanții să aleagă suprafața de capabilități.
    .help = Transmiteți parametri `cap_std::fs::Dir` împreună cu `camino::Utf8Path`/`Utf8PathBuf` prin API-urile dumneavoastră, în loc să apelați direct std::fs.

no_std_fs_operations_unknown_keys = Se ignoră cheile necunoscute { $keys } din [no_std_fs_operations] pentru no_std_fs_operations { $version } (revizia { $revision }).
    .note = Celelalte chei din această secțiune rămân valabile; cheile nerecunoscute nu au niciun efect.
    .help = Actualizați suita Whitaker la o revizie care înțelege aceste chei sau eliminați-le din dylint.toml.

no_std_fs_operations_discarded_section = Se elimină configurația [no_std_fs_operations] pentru no_std_fs_operations { $version } (revizia { $revision }): { $error }
    .note = Nu se aplică nicio excludere; lintul folosește setările implicite.
    .help = Corectați valoarea nevalidă din dylint.toml sau actualizați suita Whitaker astfel încât să înțeleagă cheia.
