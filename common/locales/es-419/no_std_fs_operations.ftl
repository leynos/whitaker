## Restringir las operaciones de std::fs para imponer una E/S basada en capacidades.

no_std_fs_operations = La operación de std::fs `{ $operation }` elude la política de sistema de archivos basada en capacidades.
    .note = std::fs actúa sobre el directorio de trabajo ambiental; acepte en su lugar descriptores `cap_std::fs::Dir` y rutas de `camino` para que quienes llaman elijan la superficie de capacidades.
    .help = Pase parámetros `cap_std::fs::Dir` junto con `camino::Utf8Path`/`Utf8PathBuf` a través de sus API en lugar de llamar a std::fs directamente.

no_std_fs_operations_unknown_keys = Se ignoran las claves desconocidas { $keys } en [no_std_fs_operations] para no_std_fs_operations { $version } (revisión { $revision }).
    .note = Las demás claves de esta sección siguen aplicándose; las claves no reconocidas no tienen efecto.
    .help = Actualice la suite Whitaker a una revisión que entienda estas claves o elimínelas de dylint.toml.

no_std_fs_operations_discarded_section = Se descarta la configuración [no_std_fs_operations] para no_std_fs_operations { $version } (revisión { $revision }): { $error }
    .note = No se aplica ninguna exclusión; el lint usa su configuración predeterminada.
    .help = Corrija el valor no válido en dylint.toml o actualice la suite Whitaker para que entienda la clave.
