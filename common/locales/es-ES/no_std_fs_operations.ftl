## Restringir las operaciones de std::fs para imponer una E/S basada en capacidades.

no_std_fs_operations = La operación de std::fs `{ $operation }` elude la política de sistema de ficheros basada en capacidades.
    .note = std::fs actúa sobre el directorio de trabajo ambiental; acepta en su lugar descriptores `cap_std::fs::Dir` y rutas de `camino` para que quienes llaman elijan la superficie de capacidades.
    .help = Pasa parámetros `cap_std::fs::Dir` junto con `camino::Utf8Path`/`Utf8PathBuf` a través de tus API en lugar de llamar a std::fs directamente.
