## Restreindre les opérations std::fs pour imposer des E/S fondées sur les capacités.

no_std_fs_operations = L’opération std::fs `{ $operation }` contourne la politique d’accès au système de fichiers fondée sur les capacités.
    .note = std::fs agit sur le répertoire de travail ambiant ; acceptez plutôt des descripteurs `cap_std::fs::Dir` et des chemins `camino` afin que les appelants choisissent la surface de capacités.
    .help = Transmettez des paramètres `cap_std::fs::Dir` ainsi que `camino::Utf8Path`/`Utf8PathBuf` à travers vos API au lieu d’appeler std::fs directement.

no_std_fs_operations_unknown_keys = Les clés inconnues { $keys } de [no_std_fs_operations] pour no_std_fs_operations { $version } (révision { $revision }) sont ignorées.
    .note = Les autres clés de cette section restent appliquées ; les clés non reconnues n’ont aucun effet.
    .help = Mettez à jour la suite Whitaker vers une révision qui comprend ces clés, ou supprimez-les de dylint.toml.

no_std_fs_operations_discarded_section = La configuration [no_std_fs_operations] pour no_std_fs_operations { $version } (révision { $revision }) est écartée : { $error }
    .note = Aucune exclusion ne s’applique ; le lint utilise ses réglages par défaut.
    .help = Corrigez la valeur invalide dans dylint.toml, ou mettez à jour la suite Whitaker pour qu’elle comprenne la clé.
