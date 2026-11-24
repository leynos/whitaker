## Restreindre les opérations std::fs pour imposer des E/S fondées sur les capacités.

no_std_fs_operations = L’opération std::fs `{ $operation }` contourne la politique d’accès au système de fichiers fondée sur les capacités.
    .note = std::fs agit sur le répertoire de travail ambiant ; acceptez plutôt des descripteurs `cap_std::fs::Dir` et des chemins `camino` afin que les appelants choisissent la surface de capacités.
    .help = Transmettez des paramètres `cap_std::fs::Dir` ainsi que `camino::Utf8Path`/`Utf8PathBuf` à travers vos API au lieu d’appeler std::fs directement.
